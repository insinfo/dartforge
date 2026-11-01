//! Our analyzer over a set of files: loading, outline, inference and the
//! diagnostics of each file, already with codes (through [`crate::bridge`]).
//!
//! A set of files becomes **a single program**: a synthetic in-memory entry
//! (`__dartforge_analise__.dart`, through the `Generation` of `elements`) imports
//! each library with its own prefix. Each file is still its own library, as in
//! `dart analyze` of a directory, and the SDK and the packages are loaded and
//! typed once per batch instead of once per file. Until the semantic database
//! (plan B2) exists, this is the only per-directory analysis driver.
//!
//! **Attributing to a file.** The `Diagnostic` of `types` does not say yet
//! which unit it is in (request T1). Until then: bodies, one inference pass
//! per library of the batch (what it adds after the diagnostics of the
//! initializers belongs to that library); initializers, in the order of
//! `program.variables`, each one in the first initializer (from the previous
//! one on) that contains it; outline, the unit with a node (expression,
//! statement, type or pattern) of the exact range, or the batch unit whose
//! top-level declaration contains it. Cases with more than one candidate are
//! counted in [`AnalysisResult::ambiguous`] — the scoreboard shows them.

use crate::bridge;
use dartforge_diagnostics::{Diagnostic, Span, codes};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::DirectiveKind;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Name of the synthetic entry (it never exists on disk).
pub const ENTRY: &str = "__dartforge_analise__.dart";

/// The SDK, loaded once and reused by every analysis.
pub struct Engine {
    sdk: SdkLayout,
    /// Every `dart:` library the analyzer knows: those of any section of
    /// `libraries.json` (the analyzer is not limited to DDC).
    sdk_libraries: BTreeSet<String>,
}

/// What a resident analysis process keeps between analyses
/// ([`Engine::analyze_resident`]): the interner and the analyzed units —
/// source and tree — of every file the loads read, SDK included. The next
/// load takes them back instead of reading, lexing and parsing them again;
/// only the units whose text changed are analyzed again.
///
/// It also keeps the last analysis's loaded program with its scopes and its
/// outline types ([`World`]). When the next analysis is of the same batch and
/// only bodies of the batch's files changed (the same API signature, nothing
/// else in memory or on disk), those units are swapped in place
/// (`dartforge_elements::incremental::swap_unit`) and the analysis starts
/// from the outline: the load, the scopes and the outline of the whole
/// program are not redone. Everything from the initializers on (inference,
/// the verifiers, the diagnostics) is redone by each analysis.
///
/// The cost is the trees of the whole program kept alive between analyses
/// ([`Resident::source_bytes`] measures the source; the trees are a few
/// times that); the owner decides when that is too much and calls
/// [`Resident::clear`].
#[derive(Default)]
pub struct Resident {
    interner: dartforge_intern::Interner,
    units: dartforge_elements::unit_cache::AnalyzedUnitCache,
    /// The in-memory texts of the previous analysis (canonical keys): one that
    /// is gone now counts from the disk again.
    texts: HashMap<PathBuf, u64>,
    /// The program and outline of the previous analysis, while it can serve
    /// the next one.
    world: Option<World>,
    /// Analyses that started from the kept [`World`] (tests and measurement).
    pub reused_worlds: usize,
    /// Analyses that loaded over the kept world's frozen prefix (tests and
    /// measurement).
    pub onto_worlds: usize,
    /// The package's libraries the last of those loads kept.
    pub kept_libraries: usize,
    /// The files of the package the batches belong to ([`Resident::set_context`]):
    /// the program of every analysis imports them all, so that the batches of
    /// one package share one program and the [`World`] serves them all.
    context: Vec<PathBuf>,
    /// The list as given to the last [`Resident::set_context`].
    context_given: Vec<PathBuf>,
    /// Whether each context file is a part, with the stamp it had when read.
    context_parts: HashMap<PathBuf, (dartforge_elements::unit_cache::FileStamp, bool)>,
    /// Per package root, the sizes of its programs that decide whether the
    /// whole package pays ([`Resident::context_pays`]).
    program_bytes: HashMap<PathBuf, ProgramBytes>,
}

/// The source bytes (outside the SDK) of a package's programs: the largest
/// one a small batch (edited files, [`EDITED_FILES`]) imports, and the
/// largest program loaded (the whole package's, once a batch covered it).
#[derive(Default, Clone, Copy)]
struct ProgramBytes {
    edited: Option<usize>,
    package: usize,
}

/// A batch of at most this many files is the edited files' (a document and
/// its parts, Dart's priority files): its own program decides whether the
/// whole package pays ([`Resident::context_pays`]).
const EDITED_FILES: usize = 4;

/// How much larger than the edited files' own program the whole package's may
/// be for the resident to load the whole package with every batch: the extra
/// cost of each load stays small, and the dependents of an API edit start from
/// the program the edited file's analysis built. On `new_sali/frontend` and
/// `new_sali/backend` the package's program is ~1.3× the edited file's; on
/// `limitless_ui` (tests that pull `package:test` in) ~2.2×, and there the
/// whole package made the edited file's own diagnostic ~2× slower.
const CONTEXT_RATIO: f64 = 1.5;

impl Resident {
    /// Whether the next analysis of the package at `root` loads the whole
    /// package (the context): not until an edited file's own program is
    /// known; then when the largest program loaded is at most
    /// [`CONTEXT_RATIO`] times the largest edited file's. The decision is the
    /// package's, not the batch's, so that the batches of one job share the
    /// program. `DARTFORGE_RESIDENT_CONTEXT=always` or `never` forces it.
    fn context_pays(&self, root: &Path) -> bool {
        if self.context.is_empty() {
            return false;
        }
        match std::env::var("DARTFORGE_RESIDENT_CONTEXT").as_deref() {
            Ok("always") => return true,
            Ok("never") => return false,
            _ => {}
        }
        let bytes = self.program_bytes.get(root).copied().unwrap_or_default();
        let Some(edited) = bytes.edited else {
            return false;
        };
        bytes.package as f64 <= edited as f64 * CONTEXT_RATIO
    }
}

/// The source bytes outside the SDK of the libraries `files` import and
/// export in `program`, transitively (their own program, even when `program`
/// has the whole package), and of the whole `program`.
fn program_bytes(program: &Program, files: &[PathBuf]) -> (usize, usize) {
    let bytes = |l: LibraryId| -> usize {
        program
            .library(l)
            .units
            .iter()
            .map(|u| program.unit(*u).source.len())
            .sum()
    };
    let batch: HashSet<PathBuf> = files.iter().map(|f| key(f)).collect();
    let mut seen: HashSet<LibraryId> = HashSet::new();
    let mut stack: Vec<LibraryId> = program
        .units
        .iter()
        .filter(|u| u.path.as_deref().is_some_and(|p| batch.contains(&key(p))))
        .map(|u| u.library)
        .collect();
    let mut own = 0;
    while let Some(l) = stack.pop() {
        let library = program.library(l);
        if library.is_sdk || !seen.insert(l) {
            continue;
        }
        own += bytes(l);
        stack.extend(library.imports.iter().map(|i| i.library));
        stack.extend(library.exports.iter().map(|e| e.library));
    }
    let all = (0..program.libraries.len())
        .map(|i| LibraryId(i as u32))
        .filter(|l| !program.library(*l).is_sdk)
        .map(bytes)
        .sum();
    (own, all)
}

/// What [`Resident::take_world`] gives back.
enum Taken {
    /// No kept world (or one that cannot serve: its units went back to the
    /// unit cache).
    Nothing,
    /// The kept world, with the batch's changed bodies swapped in.
    Swapped(World),
    /// The kept world, whose program lost what came after its frozen prefix
    /// (the units went back to the unit cache): the analysis loads the rest
    /// over the prefix and resolves its outline over the prefix's.
    Onto(World, dartforge_elements::frozen::Checkpoint),
}

/// How many loads over a cut a world takes before a full reload: none by
/// default, since each load collects the type table
/// (`dartforge_types::collect_types`); `DARTFORGE_LSP_ONTO_LIMIT=N` brings
/// the full reload back every N loads (a safety net, to measure).
fn max_onto_loads() -> usize {
    std::env::var("DARTFORGE_LSP_ONTO_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(usize::MAX)
}

/// The program of an analysis with what the analysis builds over it before
/// the initializers: the type table, the core types, the outline types and
/// their diagnostics, the simple bounds and the load's diagnostics.
struct World {
    program: Program,
    table: dartforge_types::TypeTable,
    core: dartforge_types::CoreTypes,
    outline: dartforge_types::OutlineTypes,
    outline_diags: Vec<Diagnostic>,
    outline_units: Vec<UnitId>,
    simple_bounds: dartforge_analysis::simple_bounds::SimpleBounds,
    load_diags: Vec<Diagnostic>,
    /// The diagnostics of the initializer passes, each with the unit it was
    /// registered in: they depend on the signatures and initializers only,
    /// which a reuse keeps. `None` before the first pass, or when one of them
    /// has no unit (its place could not be followed through a swap).
    init: Option<(Vec<Diagnostic>, Vec<Option<UnitId>>)>,
    /// What the program was loaded for: the package root, the package
    /// configuration and the libraries the synthetic entry imports (the
    /// batch's and the context's).
    root: PathBuf,
    packages: Option<PathBuf>,
    imports: BTreeSet<String>,
    /// The units whose syntax diagnostics are in `load_diags` (those the load
    /// parsed for the batch, and the swapped ones); a batch unit outside it is
    /// parsed again for them when the world serves another batch.
    syntax_known: HashSet<PathBuf>,
    /// The loads over the program's frozen prefix since the last full load.
    onto_loads: usize,
}

/// Where an old offset of a text goes in a new one that differs from it in
/// one contiguous range (the common prefix and suffix stay); `None` inside
/// the range.
fn shift(old: &str, new: &str) -> impl Fn(usize) -> Option<usize> {
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let room = a.len().min(b.len()) - prefix;
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(room)
        .take_while(|(x, y)| x == y)
        .count();
    let old_end = a.len() - suffix;
    let delta = b.len() as isize - a.len() as isize;
    move |o: usize| {
        if o <= prefix {
            Some(o)
        } else if o >= old_end {
            Some((o as isize + delta) as usize)
        } else {
            None
        }
    }
}

/// The `// @dart = x.y` marker of a text (its language version).
fn version_marker(text: &str) -> Option<&str> {
    text.lines()
        .take_while(|l| {
            let t = l.trim_start();
            t.is_empty() || t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')
        })
        .find(|l| l.contains("@dart"))
}

impl Resident {
    pub fn new() -> Resident {
        Resident::default()
    }

    /// The source bytes of the units kept.
    pub fn source_bytes(&self) -> usize {
        self.units.source_bytes()
            + self.world.as_ref().map_or(0, |w| {
                w.program
                    .units
                    .iter()
                    .map(|u| u.source.len())
                    .sum::<usize>()
            })
    }

    /// The units kept.
    pub fn units(&self) -> usize {
        self.units.len() + self.world.as_ref().map_or(0, |w| w.program.units.len())
    }

    /// The kept [`World`] for an analysis of the batch `files` (canonical)
    /// with `texts` whose entry imports `imports`, with the batch's changed
    /// bodies swapped in; `None` when it cannot serve (its units then go back
    /// to the unit cache, and the analysis loads as usual).
    fn take_world(
        &mut self,
        root: &Path,
        packages: Option<&Path>,
        imports: &BTreeSet<String>,
        files: &[PathBuf],
        texts: &HashMap<PathBuf, String>,
    ) -> Taken {
        let Some(mut world) = self.world.take() else {
            return Taken::Nothing;
        };
        // A refused swap may leave the syntax diagnostics taken.
        let load_diags = world.load_diags.clone();
        if self.reuse(&mut world, root, packages, imports, files, texts) {
            return Taken::Swapped(world);
        }
        world.load_diags = load_diags;
        // An API change in the batch: the program's frozen prefix (the SDK
        // and the dependencies), the package's libraries that do not depend
        // on what changed, and what was computed over them stay; the rest is
        // loaded again over them. A full reload now and then drops what the
        // type table kept of the earlier loads.
        if let Some(frozen) = world.program.frozen
            && world.onto_loads < max_onto_loads()
            && self.same_prefix(&world, root, packages, texts)
        {
            let at = self.cut(&world, imports, texts, frozen);
            let after: Vec<dartforge_elements::model::Unit> =
                world.program.units.drain(at.units..).collect();
            self.units.reclaim_units(after);
            self.kept_libraries = at.libraries - frozen.libraries;
            return Taken::Onto(world, at);
        }
        self.units.reclaim(world.program);
        Taken::Nothing
    }

    /// Where the kept program is cut for a load over it: at the start of the
    /// group (`Program::groups`) of the first library with a unit whose text
    /// changed (in the texts or on the disk), or of the entry when the batch
    /// imports other files; never before the frozen prefix. A file the
    /// program does not have yet comes in through one that changed.
    fn cut(
        &self,
        w: &World,
        imports: &BTreeSet<String>,
        texts: &HashMap<PathBuf, String>,
        frozen: dartforge_elements::frozen::Checkpoint,
    ) -> dartforge_elements::frozen::Checkpoint {
        use std::hash::{Hash, Hasher};
        let digest = |t: &str| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut h);
            h.finish()
        };
        let mut changed: HashSet<PathBuf> = self.units.changed().iter().map(|p| key(p)).collect();
        changed.extend(
            texts
                .iter()
                .filter(|(k, t)| self.texts.get(*k) != Some(&digest(t)))
                .map(|(k, _)| k.clone()),
        );
        changed.extend(
            self.texts
                .keys()
                .filter(|k| !texts.contains_key(*k))
                .cloned(),
        );
        let program = &w.program;
        let entry = program
            .entry
            .map_or(program.libraries.len(), |e| e.0 as usize);
        let mut first = if w.imports != *imports {
            entry
        } else {
            program.libraries.len()
        };
        for u in &program.units[frozen.units.min(program.units.len())..] {
            if u.path.as_deref().is_some_and(|p| changed.contains(&key(p))) {
                first = first.min(u.library.0 as usize);
            }
        }
        if first >= program.libraries.len() {
            first = entry.min(program.libraries.len().saturating_sub(1));
        }
        let start = program
            .group_start(LibraryId(first as u32))
            .max(frozen.libraries);
        program.checkpoint_at(start).unwrap_or(frozen)
    }

    /// Whether `w` was built for the same package and packages, with nothing
    /// outside the package changed on the disk or in the texts: the conditions
    /// under which its frozen prefix still holds.
    fn same_prefix(
        &self,
        w: &World,
        root: &Path,
        packages: Option<&Path>,
        texts: &HashMap<PathBuf, String>,
    ) -> bool {
        use std::hash::{Hash, Hasher};
        // The package's files may change (and the batch import others): they
        // come after the prefix. What is outside it may not.
        if w.root != root || w.packages.as_deref() != packages {
            return false;
        }
        let outside = |k: &Path| !key(k).starts_with(root);
        if self.units.changed().iter().any(|p| outside(p)) {
            return false;
        }
        let digest = |t: &str| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut h);
            h.finish()
        };
        texts
            .iter()
            .all(|(k, t)| !outside(k) || self.texts.get(k) == Some(&digest(t)))
            && self
                .texts
                .keys()
                .all(|k| !outside(k) || texts.contains_key(k))
    }

    fn reuse(
        &mut self,
        w: &mut World,
        root: &Path,
        packages: Option<&Path>,
        imports: &BTreeSet<String>,
        files: &[PathBuf],
        texts: &HashMap<PathBuf, String>,
    ) -> bool {
        use std::hash::{Hash, Hasher};
        if w.root != root || w.packages.as_deref() != packages || w.imports != *imports {
            return false;
        }
        // Nothing read from the disk changed.
        if !self.units.changed().is_empty() {
            return false;
        }
        let batch: HashSet<PathBuf> = files.iter().map(|f| key(f)).collect();
        let digest = |t: &str| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut h);
            h.finish()
        };
        // The texts in memory outside the batch are the ones the program read.
        let now: HashMap<PathBuf, u64> =
            texts.iter().map(|(k, t)| (k.clone(), digest(t))).collect();
        if now
            .iter()
            .any(|(k, d)| !batch.contains(k) && self.texts.get(k) != Some(d))
            || self
                .texts
                .keys()
                .any(|k| !batch.contains(k) && !now.contains_key(k))
        {
            return false;
        }
        // The batch's units whose text is not the program's.
        let mut changes: Vec<(UnitId, String)> = Vec::new();
        for (i, u) in w.program.units.iter().enumerate() {
            let Some(p) = &u.path else { continue };
            let k = key(p);
            if !batch.contains(&k) || w.program.library(u.library).is_sdk {
                continue;
            }
            let text = match texts.get(&k) {
                Some(t) => t.clone(),
                None => match std::fs::read_to_string(p) {
                    Ok(t) => t,
                    Err(_) => return false,
                },
            };
            if text != u.source {
                if version_marker(&text) != version_marker(&u.source) {
                    return false;
                }
                changes.push((UnitId(i as u32), text));
            }
        }
        // Every kept span of a changed unit must have a place in its new text.
        let mut load_diags = std::mem::take(&mut w.load_diags);
        let mut outline_diags = w.outline_diags.clone();
        for (u, text) in &changes {
            let unit = w.program.unit(*u);
            let Some(path) = unit.path.clone() else {
                return false;
            };
            let map = shift(&unit.source, text);
            let prefix = format!("{}:", path.display());
            // The new text's syntax diagnostics take the place of the old ones
            // (where the first one was, or after the unit's last diagnostic).
            let parsed =
                dartforge_frontend::parser::parse_with(text, &mut self.interner, unit.features);
            let mut fresh: Vec<Diagnostic> = parsed
                .diagnostics
                .into_iter()
                .map(|mut d| {
                    d.message = format!("{}:{}: {}", path.display(), d.span.start, d.message);
                    d
                })
                .collect();
            let mut kept = Vec::with_capacity(load_diags.len() + fresh.len());
            let mut last_of_unit = None;
            for mut d in load_diags {
                let Some(rest) = d.message.strip_prefix(&prefix) else {
                    kept.push(d);
                    continue;
                };
                let syntactic = d.code.is_none_or(|c| {
                    c.info().error_type == dartforge_diagnostics::ErrorType::SyntacticError
                });
                if syntactic {
                    kept.append(&mut fresh);
                    last_of_unit = Some(kept.len());
                    continue;
                }
                let (Some(start), Some(end)) = (map(d.span.start), map(d.span.end)) else {
                    return false;
                };
                let old = format!("{}: ", d.span.start);
                let message = rest.strip_prefix(&old).unwrap_or(rest).to_string();
                d.span = Span { start, end };
                d.message = format!("{prefix}{start}: {message}");
                kept.push(d);
                last_of_unit = Some(kept.len());
            }
            let at = last_of_unit.unwrap_or(kept.len());
            kept.splice(at..at, fresh);
            load_diags = kept;
            for (d, du) in outline_diags.iter_mut().zip(&w.outline_units) {
                if du != u {
                    continue;
                }
                let (Some(start), Some(end)) = (map(d.span.start), map(d.span.end)) else {
                    return false;
                };
                d.span = Span { start, end };
            }
            // The initializer passes' diagnostics follow the unit too, or the
            // passes run again.
            if let Some((ds, us)) = &mut w.init {
                let mut lost = false;
                for (d, du) in ds.iter_mut().zip(us.iter()) {
                    if *du != Some(*u) {
                        continue;
                    }
                    match (map(d.span.start), map(d.span.end)) {
                        (Some(start), Some(end)) => d.span = Span { start, end },
                        _ => lost = true,
                    }
                }
                if lost {
                    w.init = None;
                }
            }
        }
        // The swaps, all or none.
        let mut done: Vec<dartforge_elements::incremental::Previous> = Vec::new();
        for (u, text) in &changes {
            let ok = match dartforge_elements::incremental::swap_unit(
                &mut w.program,
                &mut self.interner,
                *u,
                text,
                false,
            ) {
                Ok(previous) => {
                    if w.outline.remap_written_types(*u, previous.type_map()) {
                        done.push(previous);
                        true
                    } else {
                        dartforge_elements::incremental::restore_unit(&mut w.program, previous);
                        false
                    }
                }
                Err(_) => false,
            };
            if !ok {
                while let Some(previous) = done.pop() {
                    let u = previous.unit();
                    w.outline.remap_written_types(u, &previous.inverse_map());
                    dartforge_elements::incremental::restore_unit(&mut w.program, previous);
                }
                return false;
            }
        }
        // The batch's other units: their syntax diagnostics, if the load that
        // built the world served them from the unit cache.
        for (i, u) in w.program.units.iter().enumerate() {
            let Some(p) = &u.path else { continue };
            let k = key(p);
            if !batch.contains(&k) || w.syntax_known.contains(&k) {
                continue;
            }
            if changes.iter().any(|(c, _)| c.0 as usize == i) {
                continue;
            }
            let parsed =
                dartforge_frontend::parser::parse_with(&u.source, &mut self.interner, u.features);
            load_diags.extend(parsed.diagnostics.into_iter().map(|mut d| {
                d.message = format!("{}:{}: {}", p.display(), d.span.start, d.message);
                d
            }));
            w.syntax_known.insert(k);
        }
        for (u, _) in &changes {
            if let Some(p) = &w.program.unit(*u).path {
                w.syntax_known.insert(key(p));
            }
        }
        w.load_diags = load_diags;
        w.outline_diags = outline_diags;
        self.texts = now;
        self.reused_worlds += 1;
        true
    }

    /// Forgets everything: the next analysis starts from the files.
    pub fn clear(&mut self) {
        let context = std::mem::take(&mut self.context);
        let program_bytes = std::mem::take(&mut self.program_bytes);
        *self = Resident::default();
        self.context = context;
        self.program_bytes = program_bytes;
    }

    /// The files of the package whose batches the next analyses are of: the
    /// program of each one also imports them (only the batch is reported).
    /// Then any batch of the package finds the kept program, and the analysis
    /// of the dependents after an API edit starts from the program the edited
    /// file's analysis just built. Empty: only the batch.
    pub fn set_context(&mut self, files: Vec<PathBuf>) {
        // The same list as the last one (every keystroke): nothing to redo.
        if self.context_given == files {
            return;
        }
        let mut canonical_files: Vec<PathBuf> = files.iter().map(|f| canonical(f)).collect();
        canonical_files.sort();
        canonical_files.dedup();
        self.context = canonical_files;
        self.context_given = files;
    }

    /// Whether the context file `c` (not in memory) is a part, from what was
    /// read last when its date and size did not change since.
    fn context_is_part(&mut self, c: &Path) -> Option<bool> {
        use dartforge_elements::unit_cache::FileStamp;
        let stamp = FileStamp::read(c)?;
        if let Some((s, part)) = self.context_parts.get(c)
            && *s == stamp
        {
            return Some(*part);
        }
        let part = is_part(&std::fs::read_to_string(c).ok()?);
        self.context_parts.insert(c.to_path_buf(), (stamp, part));
        Some(part)
    }

    /// Drops the units that cannot be served as they are to an analysis with
    /// these in-memory `texts` of the batch `files` (canonical): those whose
    /// text is in memory now or was in the previous analysis (the editor's text
    /// changed, or the disk's counts again), those whose file changed on disk,
    /// those of the batch, whose syntax diagnostics come from parsing them, and
    /// the synthetic `entry`, whose imports are the batch's.
    fn prepare(&mut self, files: &[PathBuf], texts: &HashMap<PathBuf, String>, entry: &Path) {
        use std::hash::{Hash, Hasher};
        let digest = |t: &str| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut h);
            h.finish()
        };
        let now: HashMap<PathBuf, u64> =
            texts.iter().map(|(k, t)| (k.clone(), digest(t))).collect();
        let mut stale: Vec<PathBuf> = self.units.changed();
        stale.extend(
            self.texts
                .iter()
                .filter(|(k, d)| now.get(*k) != Some(*d))
                .map(|(k, _)| k.clone()),
        );
        stale.extend(now.keys().filter(|k| !self.texts.contains_key(*k)).cloned());
        stale.extend(files.iter().cloned());
        stale.push(entry.to_path_buf());
        let paths = self.units.paths();
        for k in stale {
            for p in paths.iter().filter(|p| key(p) == key(&k)) {
                self.units.invalidate(p);
            }
        }
        self.texts = now;
    }
}

/// Gives the analysis's [`World`] back to the [`Resident`] when the analysis
/// ends, on every path (completed or cancelled): the next analysis may start
/// from it.
struct Reclaim<'a> {
    world: Option<World>,
    slot: Option<&'a mut Option<World>>,
}

impl Drop for Reclaim<'_> {
    fn drop(&mut self) {
        if let (Some(world), Some(slot)) = (self.world.take(), self.slot.as_deref_mut()) {
            *slot = Some(world);
        }
    }
}

/// The diagnostics of a file, with its text (to convert positions).
#[derive(Debug, Default)]
pub struct FileAnalysis {
    pub text: String,
    pub diags: Vec<Diagnostic>,
    /// How many of the first `diags` are syntactic (always published).
    pub syntactic_count: usize,
    /// The findings of the lint rules that need the resolved program (the
    /// hierarchy, the types): they only come out if the rule is enabled in the
    /// options.
    pub semantic_lints: Vec<SemanticLint>,
    /// The reports of every lint rule over the program's tree, with the
    /// engine's semantics (`dartforge_analysis::lints::run_with`): the
    /// publisher filters by the enabled rules. `None` when the file is not a
    /// unit of the resolved program.
    pub lint_reports: Option<Vec<dartforge_analysis::lints::rules::LintReport>>,
}

/// A lint rule finding computed with the resolved program.
#[derive(Debug, Clone)]
pub struct SemanticLint {
    /// The code's unique name (`LinterLintCode`), which is also the key in
    /// `dartforge_analysis::lints::codes_g::ALL`.
    pub unique: &'static str,
    pub span: dartforge_diagnostics::Span,
    pub args: Vec<String>,
}

/// The phases of `LibraryAnalyzer.analyze()` (INFRA §1.2), in the order they
/// report: the sort key of each file's final list.
pub mod phase {
    pub const SCANNER: u16 = 0;
    pub const PARSER: u16 = 10;
    pub const DIRECTIVES: u16 = 20;
    pub const RESOLUTION_VISITOR: u16 = 31;
    pub const SCOPE_RESOLVER: u16 = 32;
    pub const RESOLVER_VISITOR: u16 = 33;
    pub const CONSTANTS: u16 = 40;
    pub const CONSTANT_VERIFIER: u16 = 51;
    pub const INHERITANCE_OVERRIDE: u16 = 52;
    pub const ERROR_VERIFIER: u16 = 53;
    pub const FFI_VERIFIER: u16 = 54;
    pub const MEMBER_DUPLICATE_DEFINITION: u16 = 61;
    pub const CONSTRUCTOR_FIELDS: u16 = 62;
    pub const UNICODE_TEXT: u16 = 71;
    pub const DEAD_CODE: u16 = 72;
    pub const BEST_PRACTICES: u16 = 73;
    pub const OVERRIDE_VERIFIER: u16 = 74;
    pub const REDECLARE_VERIFIER: u16 = 75;
    pub const TODO_FINDER: u16 = 76;
    pub const LANGUAGE_VERSION: u16 = 77;
    pub const IMPORTS_VERIFIER: u16 = 78;
    pub const UNUSED_LOCAL_ELEMENTS: u16 = 79;
    pub const SDK_CONSTRAINT: u16 = 80;
    pub const IGNORE_VALIDATOR: u16 = 100;
}

/// The time of each phase of an analysis on standard error, with
/// `DARTFORGE_ENGINE_TIMES=1` (the `analyze_project` example's breakdown).
struct PhaseTimes {
    last: Option<std::time::Instant>,
}

impl PhaseTimes {
    fn from_env() -> Self {
        let on = std::env::var_os("DARTFORGE_ENGINE_TIMES").is_some_and(|v| v != "0");
        PhaseTimes {
            last: on.then(std::time::Instant::now),
        }
    }

    /// Prints the time since the previous mark under `name`.
    fn mark(&mut self, name: &str) {
        if let Some(t) = self.last {
            eprintln!("engine: {name}: {:.3} s", t.elapsed().as_secs_f64());
            self.last = Some(std::time::Instant::now());
        }
    }
}

/// Tags each report of an iterator with the emitter's phase.
fn with_phase<A, I: IntoIterator<Item = (A, Diagnostic)>>(
    f: u16,
    it: I,
) -> impl Iterator<Item = (A, Diagnostic, u16)> {
    it.into_iter().map(move |(a, d)| (a, d, f))
}

/// The result of an analysis: per normalized absolute path.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub files: BTreeMap<PathBuf, FileAnalysis>,
    /// Diagnostics with more than one candidate unit.
    pub ambiguous: usize,
    /// The phase of each report of `files[k].diags`, in the same order.
    phases: BTreeMap<PathBuf, Vec<u16>>,
}

impl AnalysisResult {
    /// Reports `d` in file `k`, in phase `f`.
    fn report(&mut self, k: &Path, d: Diagnostic, f: u16) {
        if let Some(a) = self.files.get_mut(k) {
            a.diags.push(d);
            self.phases.entry(k.to_path_buf()).or_default().push(f);
        }
    }

    /// Each file's list (except the syntactic ones, which come first) in
    /// phase order, stable.
    fn sort_by_phases(&mut self) {
        for (k, a) in self.files.iter_mut() {
            let phases = self.phases.remove(k).unwrap_or_default();
            let n = a.syntactic_count.min(a.diags.len());
            let rest: Vec<Diagnostic> = a.diags.drain(n..).collect();
            let mut with: Vec<(u16, usize, Diagnostic)> = rest
                .into_iter()
                .enumerate()
                .map(|(i, d)| {
                    (
                        phases
                            .get(n + i)
                            .copied()
                            .unwrap_or(phase::RESOLVER_VISITOR),
                        i,
                        d,
                    )
                })
                .collect();
            with.sort_by_key(|(f, i, _)| (*f, *i));
            a.diags.extend(with.into_iter().map(|(_, _, d)| d));
        }
    }
}

/// Normalized path (lexical, without `\\?\`), the comparison key of files.
pub fn key(p: &Path) -> PathBuf {
    dartforge_elements::generated::key(p)
}

/// The path as the file system spells it (case and links resolved), as the
/// loading sees the files; for a file that does not exist yet (an unsaved
/// document), its directory's canonical path with the name.
fn canonical(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).ok().or_else(|| {
        let parent = p.parent()?.canonicalize().ok()?;
        Some(parent.join(p.file_name()?))
    });
    dartforge_elements::config::without_verbatim(c.unwrap_or_else(|| p.to_path_buf()))
}

/// Is the file a part (`part of`)? Looks at the directives before the first
/// declaration.
pub fn is_part(text: &str) -> bool {
    let mut in_block = false;
    for line in text.lines() {
        let t = line.trim();
        if in_block {
            if t.contains("*/") {
                in_block = false;
            }
            continue;
        }
        if t.is_empty() || t.starts_with("//") || t.starts_with('@') || t.starts_with("#!") {
            continue;
        }
        if t.starts_with("/*") {
            in_block = !t.contains("*/");
            continue;
        }
        return t.starts_with("part of");
    }
    false
}

/// The path of a directive URI written in `base` (relative, or `package:`
/// through `package_config.json`); `dart:` and other schemes have none.
fn resolve_directive_uri(
    uri: &str,
    base: &Path,
    config: Option<&dartforge_elements::PackageConfig>,
) -> Option<PathBuf> {
    if let Some(rest) = uri.strip_prefix("package:") {
        let (package, rel) = rest.split_once('/')?;
        let dir = config?
            .package_dirs
            .iter()
            .find(|(n, _)| n == package)
            .map(|(_, d)| d.clone())?;
        return Some(dir.join(rel));
    }
    if uri.contains(':') {
        return None;
    }
    // `/x.dart` in a `lib/` library (a `package:` URI): the package's root,
    // like `resolver_relativo_a_package` while loading.
    if uri.starts_with('/')
        && !uri.starts_with("//")
        && let Some(dir) = config.and_then(|c| {
            c.package_dirs
                .iter()
                .find(|(_, d)| base.starts_with(d))
                .map(|(_, d)| d.clone())
        })
    {
        return Some(dir.join(uri.trim_start_matches('/')));
    }
    Some(base.join(uri))
}

/// Does part `a` have a known library (`PartOfUriKnownFileKind.library` /
/// `PartOfNameFileKind.library`)? That of the `part of 'uri'`, which exists
/// and includes it through a `part` directive; or, by name, a library of the
/// batch with that name that includes it.
fn part_has_library(
    a: &Path,
    text: &str,
    batch: &[(usize, &PathBuf, String)],
    config: Option<&dartforge_elements::PackageConfig>,
    texts: &HashMap<PathBuf, String>,
) -> bool {
    let mut names = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(text, &mut names);
    let base = a.parent().map(Path::to_path_buf).unwrap_or_default();
    let target = key(a);
    // Does library `lib` (text) include `target` through `part`?
    let includes = |lib_path: &Path, lib_text: &str| -> bool {
        let mut n2 = dartforge_intern::Interner::new();
        let lib = dartforge_frontend::parser::parse(lib_text, &mut n2);
        let lib_base = lib_path.parent().map(Path::to_path_buf).unwrap_or_default();
        lib.unit.directives.iter().any(|d| match &d.kind {
            DirectiveKind::Part { uri } => dartforge_elements::load::string_lit_value(uri)
                .and_then(|u| resolve_directive_uri(&u, &lib_base, config))
                .is_some_and(|c| key(&c) == target),
            _ => false,
        })
    };
    for d in &parsed.unit.directives {
        let DirectiveKind::PartOf { uri, name } = &d.kind else {
            continue;
        };
        if let Some(lit) = uri {
            let Some(u) = dartforge_elements::load::string_lit_value(lit) else {
                return false;
            };
            let Some(lib_path) = resolve_directive_uri(&u, &base, config) else {
                return false;
            };
            let lib_text = texts
                .get(&key(&lib_path))
                .cloned()
                .or_else(|| std::fs::read_to_string(&lib_path).ok());
            return lib_text.is_some_and(|t| !is_part(&t) && includes(&lib_path, &t));
        }
        // `part of a.b;`: a library of the batch with that name that includes it.
        let wanted: Vec<String> = name
            .iter()
            .map(|n| names.resolve(n.sym).to_string())
            .collect();
        return batch.iter().any(|(_, c, t)| {
            if is_part(t) {
                return false;
            }
            let mut n3 = dartforge_intern::Interner::new();
            let lib = dartforge_frontend::parser::parse(t, &mut n3);
            let lib_name = lib.unit.directives.iter().find_map(|d| match &d.kind {
                DirectiveKind::Library { name } => Some(
                    name.iter()
                        .map(|n| n3.resolve(n.sym).to_string())
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            });
            lib_name.as_ref() == Some(&wanted) && includes(c, t)
        });
    }
    false
}

/// The library that includes part `a` when it is not in the batch: the one of
/// the `part of 'uri'`, or, by name, a library **of the same folder** with
/// that name that includes it (`PartOfNameFileKind.discoverLibraries` only
/// reads the siblings; the first by path). `None` if the library is in the
/// batch or is not found this way (the part goes on as its own library).
fn library_outside_batch(
    a: &Path,
    text: &str,
    root: &Path,
    batch: &[(usize, &PathBuf, String)],
    config: Option<&dartforge_elements::PackageConfig>,
    texts: &HashMap<PathBuf, String>,
) -> Option<PathBuf> {
    let mut names = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(text, &mut names);
    let base = a.parent().map(Path::to_path_buf).unwrap_or_default();
    let target = key(a);
    let in_batch = |c: &Path| batch.iter().any(|(_, x, _)| key(x) == key(c));
    let read = |c: &Path| {
        texts
            .get(&key(c))
            .cloned()
            .or_else(|| std::fs::read_to_string(c).ok())
    };
    let includes = |lib_path: &Path, lib_text: &str| -> bool {
        let mut n2 = dartforge_intern::Interner::new();
        let lib = dartforge_frontend::parser::parse(lib_text, &mut n2);
        let lib_base = lib_path.parent().map(Path::to_path_buf).unwrap_or_default();
        lib.unit.directives.iter().any(|d| match &d.kind {
            DirectiveKind::Part { uri } => dartforge_elements::load::string_lit_value(uri)
                .and_then(|u| resolve_directive_uri(&u, &lib_base, config))
                .is_some_and(|c| key(&c) == target),
            _ => false,
        })
    };
    let d = parsed
        .unit
        .directives
        .iter()
        .find(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))?;
    let DirectiveKind::PartOf { uri, name } = &d.kind else {
        return None;
    };
    if let Some(lit) = uri {
        let u = dartforge_elements::load::string_lit_value(lit)?;
        let path = resolve_directive_uri(&u, &base, config)?;
        let t = read(&path)?;
        return (!in_batch(&path) && !is_part(&t) && includes(&path, &t)).then_some(path);
    }
    let wanted: Vec<String> = name
        .iter()
        .map(|n| names.resolve(n.sym).to_string())
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let _ = root;
    let mut siblings: Vec<PathBuf> = std::fs::read_dir(&base)
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "dart"))
                .collect()
        })
        .unwrap_or_default();
    siblings.sort();
    for c in siblings {
        if in_batch(&c) || key(&c) == target {
            continue;
        }
        let Some(t) = read(&c) else { continue };
        if is_part(&t) || !t.contains(wanted.last().map(String::as_str).unwrap_or("")) {
            continue;
        }
        let mut n3 = dartforge_intern::Interner::new();
        let lib = dartforge_frontend::parser::parse(&t, &mut n3);
        let lib_name = lib.unit.directives.iter().find_map(|d| match &d.kind {
            DirectiveKind::Library { name } => Some(
                name.iter()
                    .map(|n| n3.resolve(n.sym).to_string())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        });
        if lib_name.as_ref() == Some(&wanted) && includes(&c, &t) {
            return Some(c);
        }
    }
    None
}

/// Generated-file suffixes (`file_paths.isGenerated`, analyzer 6.11.0).
fn is_generated(uri: &str) -> bool {
    [
        ".g.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbserver.dart",
        ".pbjson.dart",
        ".template.dart",
    ]
    .iter()
    .any(|s| uri.ends_with(s))
}

/// Does file `target` exist as a `build_runner` output
/// (`.dart_tool/build/generated/<package>/<rel>`, from `lib/` or any other
/// folder of the package)? The analyzer sees them.
fn generated_by_build(c: &dartforge_elements::PackageConfig, target: &Path) -> bool {
    c.generated_in_place_of(target).is_some()
        || c.package_dirs.iter().any(|(name, dir)| {
            target
                .strip_prefix(dir)
                .ok()
                .and_then(|rel| {
                    c.generated_path(
                        name,
                        &rel.to_string_lossy()
                            .replace(std::path::MAIN_SEPARATOR, "/"),
                    )
                })
                .is_some()
        })
}

impl Engine {
    pub fn new(sdk_lib: &Path) -> Result<Engine, String> {
        let mut sdk = SdkLayout::load(sdk_lib, "dartdevc")?;
        // `dart analyze` uses no compilation profile: it sees every public
        // library of the platform, from source and without patches. Those the
        // DDC profile lacks (`dart:ffi`, `dart:mirrors`, `dart:cli`,
        // `dart:nativewrappers`) come from the VM sections, without their
        // patches; without this, `import 'dart:ffi'` did not resolve and every
        // `Pointer`, `Struct`… became `undefined_class`.
        for section in ["vm_common", "vm"] {
            if let Ok(vm) = SdkLayout::load(sdk_lib, section) {
                for (name, mut lib) in vm.libraries {
                    if !name.starts_with('_') && !sdk.libraries.contains_key(&name) {
                        lib.patches.clear();
                        sdk.libraries.insert(name, lib);
                    }
                }
            }
        }
        // The rest of what the analyzer sees (`libraries.dart`, without
        // patches): the internal ones that none of the profiles above bring
        // (`dart:_wasm`), so that `import 'dart:_wasm'` resolves and reports
        // `import_internal_library`.
        if let Ok(analyzer) = SdkLayout::load_as_analyzer(sdk_lib) {
            for (name, lib) in analyzer.libraries {
                if !sdk.libraries.contains_key(&name) && lib.path.is_file() {
                    sdk.libraries.insert(name, lib);
                }
            }
        }
        // Conditional imports: the analyzer picks the configuration whose
        // `dart.library.x` is in the context's declared variables
        // (`an611:src/dart/analysis/file_state.dart:785-800`), and `dart
        // analyze` declares none — the main URI always applies. Without a
        // "supported" library, loading also keeps the main one
        // (`import 'fake.dart' if (dart.library.js_interop) 'real.dart'`
        // resolves to `fake.dart`, as in the analyzer). An explicit empty
        // declared-variable context also keeps `== 'false'` from treating an
        // absent variable as a declared false value.
        sdk.conditional_import_variables = Some(HashMap::new());
        for lib in sdk.libraries.values_mut() {
            lib.supported = false;
        }
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(sdk_lib.join("libraries.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let mut sdk_libraries = BTreeSet::new();
        if let Some(o) = json.as_object() {
            for section in o.values() {
                if let Some(libs) = section.get("libraries").and_then(|l| l.as_object()) {
                    sdk_libraries.extend(libs.keys().cloned());
                }
            }
        }
        Ok(Engine { sdk, sdk_libraries })
    }

    /// The SDK from `DARTFORGE_SDK_LIB`/`DART_SDK`/`PATH`, or the machine's
    /// default 3.6.2.
    pub fn discover() -> Result<Engine, String> {
        let lib =
            SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
        Engine::new(&lib)
    }

    /// Analyzes `files` (absolute, inside `root`), each as its own library.
    /// `packages` is the `package_config.json`.
    pub fn analyze(
        &self,
        root: &Path,
        files: &[PathBuf],
        packages: Option<&Path>,
    ) -> AnalysisResult {
        self.analyze_with(root, files, packages, &HashMap::new(), &|| false)
            .expect("without cancellation, the analysis always completes")
    }

    /// Like [`Engine::analyze`], with in-memory texts and cancellation.
    ///
    /// `texts` (keyed by the path's [`key`]) takes precedence over the disk,
    /// both for `files` and for any file the loading reaches: it is what the
    /// LSP passes with the open documents not yet saved. `cancelled` is
    /// consulted between phases (loading, verifiers, each library in body
    /// inference); when it answers true, the analysis stops there and returns
    /// `None` — the LSP discards it because the text changed. The engine
    /// retains nothing between calls.
    ///
    /// The paths may differ from the files' own spelling in case (macOS,
    /// Windows) or in links: the loading canonicalizes them, so the root, the
    /// files and the texts are canonicalized here too and the result comes
    /// back keyed by the paths as given.
    pub fn analyze_with(
        &self,
        root: &Path,
        files: &[PathBuf],
        packages: Option<&Path>,
        texts: &HashMap<PathBuf, String>,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<AnalysisResult> {
        self.analyze_kept(root, files, packages, texts, cancelled, None)
    }

    /// Like [`Engine::analyze_with`], reusing what `resident` kept from the
    /// previous analyses (the analyzed units of the files whose text did not
    /// change) and keeping this one's for the next. The result is the same as
    /// [`Engine::analyze_with`]'s.
    pub fn analyze_resident(
        &self,
        resident: &mut Resident,
        root: &Path,
        files: &[PathBuf],
        packages: Option<&Path>,
        texts: &HashMap<PathBuf, String>,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<AnalysisResult> {
        self.analyze_kept(root, files, packages, texts, cancelled, Some(resident))
    }

    fn analyze_kept(
        &self,
        root: &Path,
        files: &[PathBuf],
        packages: Option<&Path>,
        texts: &HashMap<PathBuf, String>,
        cancelled: &dyn Fn() -> bool,
        resident: Option<&mut Resident>,
    ) -> Option<AnalysisResult> {
        let root_c = canonical(root);
        let mut given: HashMap<PathBuf, PathBuf> = HashMap::new();
        let files_c: Vec<PathBuf> = files
            .iter()
            .map(|a| {
                let c = canonical(a);
                given.insert(key(&c), key(a));
                c
            })
            .collect();
        let texts_c: HashMap<PathBuf, String> = texts
            .iter()
            .map(|(k, v)| (key(&canonical(k)), v.clone()))
            .collect();
        let packages_c = packages.map(canonical);
        let mut r = self.analyze_canonical(
            &root_c,
            &files_c,
            packages_c.as_deref(),
            &texts_c,
            cancelled,
            resident,
        )?;
        if given.iter().any(|(c, g)| c != g) {
            r.files = std::mem::take(&mut r.files)
                .into_iter()
                .map(|(k, v)| (given.get(&k).cloned().unwrap_or(k), v))
                .collect();
        }
        Some(r)
    }

    /// [`Engine::analyze_with`] over canonical paths.
    fn analyze_canonical(
        &self,
        root: &Path,
        files: &[PathBuf],
        packages: Option<&Path>,
        texts: &HashMap<PathBuf, String>,
        cancelled: &dyn Fn() -> bool,
        mut resident: Option<&mut Resident>,
    ) -> Option<AnalysisResult> {
        let mut analysis = AnalysisResult::default();
        let mut times = PhaseTimes::from_env();
        let mut own: BTreeMap<PathBuf, String> = BTreeMap::new();
        let mut imports = String::new();
        // The libraries the entry imports, for the kept world's key.
        let mut import_set: BTreeSet<String> = BTreeSet::new();
        let read: Vec<(usize, &PathBuf, String)> = files
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let text = match texts.get(&key(a)) {
                    Some(t) => t.clone(),
                    None => std::fs::read_to_string(a).ok()?,
                };
                Some((i, a, text))
            })
            .collect();
        let package_config = packages.and_then(|p| dartforge_elements::PackageConfig::load(p).ok());
        for (i, a, text) in &read {
            let (i, a, text) = (*i, *a, text.clone());
            // A part whose library is not known (the `part of` does not point
            // to a library that includes it) is analyzed as its own library
            // (`FileKind.asLibrary`; INFRA III.5 item 1).
            let part = is_part(&text);
            let as_library =
                !part || !part_has_library(a, &text, &read, package_config.as_ref(), texts);
            // A part requested without its library in the batch (the analysis
            // of a single file: the LSP, the mutations): the analyzer knows
            // all the files of the context and analyzes it inside the library
            // that includes it (`part of 'uri'` or `part of name;` of a library
            // of the package). The library joins the loading; the reports
            // still are only those of the requested files.
            if part
                && let Some(lib) =
                    library_outside_batch(a, &text, root, &read, package_config.as_ref(), texts)
            {
                let rel = lib
                    .strip_prefix(root)
                    .unwrap_or(&lib)
                    .to_string_lossy()
                    .replace('\\', "/")
                    .replace(' ', "%20");
                imports.push_str(&format!("import '{rel}' as _dfb{i};\n"));
                import_set.insert(rel);
                own.insert(key(a), text);
                continue;
            }
            if as_library {
                let rel = a
                    .strip_prefix(root)
                    .unwrap_or(a)
                    .to_string_lossy()
                    .replace('\\', "/");
                let rel: String = rel
                    .chars()
                    .map(|c| {
                        if c == ' ' {
                            "%20".to_string()
                        } else {
                            c.to_string()
                        }
                    })
                    .collect();
                imports.push_str(&format!("import '{rel}' as _dfp{i};\n"));
                import_set.insert(rel);
            }
            own.insert(key(a), text);
        }
        // The resident's context: the package's other libraries come in too
        // (reported on nothing), so that every batch of the package loads the
        // same program.
        // The whole package only where it pays (`Resident::context_pays`).
        let with_context = resident.as_ref().is_some_and(|r| r.context_pays(root));
        let context: Vec<PathBuf> = match resident.as_ref() {
            Some(r) if with_context => r.context.clone(),
            _ => Vec::new(),
        };
        for (i, c) in context.iter().enumerate() {
            let k = key(c);
            if own.contains_key(&k) {
                continue;
            }
            let part = match texts.get(&k) {
                Some(t) => is_part(t),
                None => match resident.as_deref_mut().and_then(|r| r.context_is_part(c)) {
                    Some(part) => part,
                    None => continue,
                },
            };
            if part {
                continue;
            }
            let rel = c
                .strip_prefix(root)
                .unwrap_or(c)
                .to_string_lossy()
                .replace('\\', "/")
                .replace(' ', "%20");
            if import_set.insert(rel.clone()) {
                imports.push_str(&format!("import '{rel}' as _dfc{i};\n"));
            }
        }
        // The texts move into the result: one copy of each file for the whole
        // analysis (the program keeps its own), not three.
        drop(read);
        let own: BTreeSet<PathBuf> = own
            .into_iter()
            .map(|(p, text)| {
                analysis.files.insert(
                    p.clone(),
                    FileAnalysis {
                        text,
                        diags: Vec::new(),
                        syntactic_count: 0,
                        semantic_lints: Vec::new(),
                        lint_reports: None,
                    },
                );
                p
            })
            .collect();
        if imports.is_empty() {
            return Some(analysis);
        }
        let entry = root.join(ENTRY);
        let mut c = dartforge_elements::generated::GenerationBuilder::new();
        for (path, text) in texts {
            c.put(path.clone(), text.clone(), "lsp", vec![]);
        }
        c.put(entry.clone(), imports.clone(), "parity", vec![]);
        let generation = c.finish(1).ok();
        let cache = dartforge_elements::SdkCache::open_or_build(&self.sdk, "dartdevc")
            .ok()
            .map(|(c, _)| c);
        let mut fresh = dartforge_intern::Interner::new();
        let (reused, mut onto) = match resident
            .as_deref_mut()
            .map(|r| r.take_world(root, packages, &import_set, files, texts))
        {
            Some(Taken::Swapped(w)) => (Some(w), None),
            Some(Taken::Onto(w, at)) => (None, Some((w, at))),
            _ => (None, None),
        };
        // The libraries that do not change in an editing session (the SDK and
        // what is outside the package) first: a later API change in the
        // package reloads only what comes after them.
        let frozen = |p: &Program, l: LibraryId| {
            let lib = p.library(l);
            lib.is_sdk
                || lib.units.iter().all(|u| {
                    p.unit(*u)
                        .path
                        .as_deref()
                        .is_some_and(|f| !key(f).starts_with(root))
                })
        };
        let (interner, mut units, slot, program_bytes, mut resident_counter) = match resident {
            Some(r) => {
                if reused.is_none() {
                    r.prepare(files, texts, &entry);
                }
                (
                    &mut r.interner,
                    Some(&mut r.units),
                    Some(&mut r.world),
                    Some(&mut r.program_bytes),
                    Some(&mut r.onto_worlds),
                )
            }
            None => (&mut fresh, None, None, None, None),
        };
        let from_world = reused.is_some();
        let world = match reused {
            Some(w) => w,
            None => {
                let prefix = onto.take();
                let (program, load_diags) = match prefix.as_ref() {
                    Some(_) => (Program::default(), Vec::new()),
                    // `DARTFORGE_FROZEN_PREFIX=0`: in discovery order, without
                    // a prefix (to measure).
                    None if std::env::var_os("DARTFORGE_FROZEN_PREFIX")
                        .is_some_and(|v| v == "0") =>
                    {
                        dartforge_elements::load::load_lenient_generated(
                            &entry,
                            &self.sdk,
                            packages,
                            interner,
                            cache,
                            units.as_deref_mut(),
                            generation.clone(),
                        )
                    }
                    None => dartforge_elements::load::load_lenient_frozen(
                        &entry,
                        &self.sdk,
                        packages,
                        interner,
                        cache,
                        units.as_deref_mut(),
                        generation.clone(),
                        &frozen,
                    ),
                };
                let (program, load_diags, prefix) = match prefix {
                    Some(mut w) => {
                        let base = std::mem::take(&mut w.0.program);
                        let (p, d) = dartforge_elements::load::load_lenient_onto(
                            base,
                            w.1,
                            &entry,
                            &self.sdk,
                            packages,
                            interner,
                            units.as_deref_mut(),
                            generation,
                        );
                        (p, d, Some(w))
                    }
                    None => (program, load_diags, None),
                };
                if cancelled() {
                    if let Some(u) = units {
                        u.reclaim(program);
                    }
                    return None;
                }
                times.mark(if with_context {
                    "load (with the whole package)"
                } else {
                    "load"
                });
                if let Some(bytes) = program_bytes {
                    let (own, all) = self::program_bytes(&program, files);
                    let entry = bytes.entry(root.to_path_buf()).or_default();
                    entry.package = entry.package.max(all);
                    if files.len() <= EDITED_FILES {
                        entry.edited = Some(entry.edited.unwrap_or(0).max(own));
                    }
                }
                let interner: &dartforge_intern::Interner = interner;
                let onto_loads = prefix.as_ref().map_or(0, |w| w.0.onto_loads + 1);
                let mut load_diags = load_diags;
                let mut syntax_known: HashSet<PathBuf> = own.iter().cloned().collect();
                let (table, core, outline, outline_diags, outline_units) = match prefix {
                    // The prefix's types, hierarchy and inferred initializers
                    // stay; only the outline after it is resolved. The kept
                    // libraries' diagnostics stay too.
                    Some((w, at)) => {
                        let mut table = w.table;
                        let mut core = w.core;
                        // What stays of the outline, with only the types it
                        // reaches left in the table: the earlier loads'
                        // outlines and bodies go away.
                        let mut kept = w.outline;
                        kept.truncate(&at);
                        dartforge_types::collect_types(&mut table, &mut kept, &mut core);
                        let (outline, diags, units) = dartforge_types::resolve_outline_onto(
                            &program, interner, &mut table, &core, kept, at,
                        );
                        if let Some(r) = &mut resident_counter {
                            **r += 1;
                        }
                        let kept: HashSet<String> = program.units[..at.units]
                            .iter()
                            .filter_map(|u| u.path.as_ref().map(|p| p.display().to_string()))
                            .collect();
                        let mut all_diags = Vec::with_capacity(diags.len());
                        let mut all_units = Vec::with_capacity(units.len());
                        for (d, u) in w.outline_diags.into_iter().zip(w.outline_units) {
                            if (u.0 as usize) < at.units {
                                all_diags.push(d);
                                all_units.push(u);
                            }
                        }
                        all_diags.extend(diags);
                        all_units.extend(units);
                        let mut old: Vec<Diagnostic> = w
                            .load_diags
                            .into_iter()
                            .filter(|d| {
                                d.message
                                    .split_once(':')
                                    .is_some_and(|(p, _)| kept.contains(p))
                            })
                            .collect();
                        old.append(&mut load_diags);
                        load_diags = old;
                        // By the keys of the kept units, computed once (a key
                        // asks the file system).
                        let kept_keys: HashSet<PathBuf> = program.units[..at.units]
                            .iter()
                            .filter(|u| {
                                u.path.as_deref().is_some_and(|p| {
                                    w.syntax_known
                                        .iter()
                                        .any(|k| k.ends_with(p.file_name().unwrap_or_default()))
                                })
                            })
                            .filter_map(|u| u.path.as_deref().map(key))
                            .collect();
                        syntax_known
                            .extend(w.syntax_known.into_iter().filter(|k| kept_keys.contains(k)));
                        (table, core, outline, all_diags, all_units)
                    }
                    None => {
                        let mut table = dartforge_types::TypeTable::new();
                        // Messages with the `typedef` alias and the `Never?` as written (C9).
                        table.preserve_decoration = true;
                        let core = dartforge_types::CoreTypes::init(&mut table, &program, interner);
                        let (outline, diags, units) = dartforge_types::resolve_outline_with_units(
                            &program, interner, &mut table, &core,
                        );
                        (table, core, outline, diags, units)
                    }
                };
                // `computeSimplyBounded`, once for the program.
                let simple_bounds = dartforge_analysis::simple_bounds::compute(&program);
                World {
                    program,
                    table,
                    core,
                    outline,
                    outline_diags,
                    outline_units,
                    simple_bounds,
                    load_diags,
                    init: None,
                    root: root.to_path_buf(),
                    packages: packages.map(Path::to_path_buf),
                    imports: import_set.clone(),
                    syntax_known,
                    onto_loads,
                }
            }
        };
        if from_world {
            times.mark("load (swapped into the kept program)");
        }
        let interner: &dartforge_intern::Interner = interner;
        // Without a resident, the world dies with the analysis.
        let mut kept = Reclaim {
            world: Some(world),
            slot,
        };
        let World {
            program,
            table,
            core,
            outline,
            outline_diags,
            outline_units,
            simple_bounds,
            load_diags,
            init,
            ..
        } = kept.world.as_mut().expect("loaded");
        let program: &Program = program;
        let (core, simple_bounds, outline_diags, outline_units, load_diags) = (
            &*core,
            &*simple_bounds,
            &*outline_diags,
            &*outline_units,
            &*load_diags,
        );

        // The batch's units.
        let mut unit_of: HashMap<PathBuf, UnitId> = HashMap::new();
        for (i, u) in program.units.iter().enumerate() {
            if let Some(p) = &u.path {
                let k = key(p);
                if own.contains(&k) && !program.library(u.library).is_sdk {
                    unit_of.insert(k, UnitId(i as u32));
                }
            }
        }
        let own_libs: Vec<LibraryId> = {
            let s: BTreeSet<LibraryId> =
                unit_of.values().map(|u| program.unit(*u).library).collect();
            s.into_iter().collect()
        };
        let own_units: BTreeSet<UnitId> = unit_of.values().copied().collect();

        // 1. Syntax: `elements` prefixes the path and the offset in the
        // message. What is not syntactic there (a version marker) goes in
        // later, outside the always-published range.
        let mut semantic_from_load: Vec<(PathBuf, Diagnostic)> = Vec::new();
        for d in load_diags {
            for (k, u) in &unit_of {
                let Some(p) = &program.unit(*u).path else {
                    continue;
                };
                let prefix = format!("{}:{}: ", p.display(), d.span.start);
                if let Some(msg) = d.message.strip_prefix(&prefix) {
                    let mut raw = d.clone();
                    raw.message = msg.to_string();
                    let syntactic = raw.code.is_none_or(|c| {
                        c.info().error_type == dartforge_diagnostics::ErrorType::SyntacticError
                    });
                    if syntactic {
                        analysis.report(k, bridge::encode_syntax(&raw), phase::PARSER);
                        analysis.files.get_mut(k).expect("own").syntactic_count += 1;
                    } else {
                        semantic_from_load.push((k.clone(), raw));
                    }
                    break;
                }
            }
        }
        for (k, d) in semantic_from_load {
            analysis.report(&k, d, phase::SCANNER);
        }

        // 2. Directives whose target does not exist.
        let config = packages.and_then(|p| dartforge_elements::PackageConfig::load(p).ok());
        for (k, u) in &unit_of {
            let unit = program.unit(*u);
            let base = unit
                .path
                .as_deref()
                .and_then(Path::parent)
                .unwrap_or(root)
                .to_path_buf();
            // `_resolveLibraryDocImportDirective`: the `@docImport`s of the
            // `library` directive's doc with a missing target
            // (`URI_DOES_NOT_EXIST_IN_DOC_IMPORT`).
            let tree = dartforge_analysis::Unit {
                ast: &unit.ast,
                unit: &unit.unit,
                source: &unit.source,
            };
            for i in dartforge_analysis::a_doc::library_doc_imports(tree) {
                // The interpolated URI (`DirectiveUriWithInterpolation`, on the literal).
                if i.interpolated {
                    let d = Diagnostic::with_code(
                        codes::compile_time_error::URI_WITH_INTERPOLATION,
                        i.literal,
                        [] as [&str; 0],
                    );
                    analysis.report(k, d, phase::DIRECTIVES);
                    continue;
                }
                let Some(uri) = i.uri else { continue };
                // `_reportImportDirectiveErrors`: `dart-ext:` instead of the missing target.
                if uri.starts_with("dart-ext:") {
                    let d = Diagnostic::with_code(
                        codes::compile_time_error::USE_OF_NATIVE_EXTENSION,
                        i.literal,
                        [] as [&str; 0],
                    );
                    analysis.report(k, d, phase::DIRECTIVES);
                    continue;
                }
                if texts.contains_key(&key(&base.join(&uri))) {
                    continue;
                }
                if self
                    .directive_without_target(&uri, &base, config.as_ref(), i.literal, false)
                    .is_some()
                {
                    let d = Diagnostic::with_code(
                        codes::warning::URI_DOES_NOT_EXIST_IN_DOC_IMPORT,
                        i.literal,
                        [uri.as_str()],
                    );
                    analysis.report(k, d, phase::DIRECTIVES);
                }
            }
            for dir in &unit.unit.directives {
                let (lit, part) = match &dir.kind {
                    DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => {
                        (uri, false)
                    }
                    DirectiveKind::Part { uri } => (uri, true),
                    _ => continue,
                };
                let Some(text) = dartforge_elements::load::string_lit_value(lit) else {
                    continue;
                };
                // `USE_OF_NATIVE_EXTENSION` (`library_analyzer.dart:672-684`, `:906-916`):
                // an import or export of `dart-ext:`, instead of the missing target.
                if !part && text.starts_with("dart-ext:") {
                    let d = Diagnostic::with_code(
                        codes::compile_time_error::USE_OF_NATIVE_EXTENSION,
                        lit.span,
                        [] as [&str; 0],
                    );
                    analysis.report(k, d, phase::DIRECTIVES);
                    continue;
                }
                if texts.contains_key(&key(&base.join(&text))) {
                    // An open document not saved yet: it exists for the editor.
                    continue;
                }
                if let Some(d) =
                    self.directive_without_target(&text, &base, config.as_ref(), lit.span, part)
                {
                    analysis.report(k, d, phase::DIRECTIVES);
                } else if part
                    && let Some(target_uri) =
                        self.part_without_part_of(&text, &base, config.as_ref(), program, texts)
                {
                    // `_resolvePartDirective` (`library_analyzer.dart:971-1032`):
                    // the file exists and is not a part.
                    let d = Diagnostic::with_code(
                        codes::compile_time_error::PART_OF_NON_PART,
                        lit.span,
                        [target_uri.as_str()],
                    );
                    analysis.report(k, d, phase::DIRECTIVES);
                }
            }
        }

        // The `UnusedLocalElementsVerifier` runs by element after inference
        // (`dartforge_types::phase_unused`); the by-name report is kept for
        // when there are no bodies.
        let mut deferred_privates: HashMap<
            LibraryId,
            Vec<(UnitId, dartforge_diagnostics::Diagnostic, u16)>,
        > = HashMap::new();
        // T5 (docs/ANALYZER-ESPECIFICACAO.md §G): there are no gates for syntax
        // errors (whole library, executable declaration, file); the rule is
        // that of the fragments the parser skipped, where fasta's recovery
        // would have read what ours discarded.
        // The outline comes before the declaration checks: the clauses' gate
        // (`analysis::clausulas`) uses the mixin decisions, which need types
        // (`dartforge_types::phase_mixins`).
        times.mark("directives, outline");
        /// Clears the mixin decisions on exit (on any path).
        struct ClearDecisions;
        impl Drop for ClearDecisions {
            fn drop(&mut self) {
                dartforge_analysis::clauses::set_mixin_decisions(None);
            }
        }
        let _clear_decisions = ClearDecisions;
        dartforge_analysis::clauses::set_mixin_decisions(Some(
            dartforge_types::phase_mixins::decisions(
                program, interner, table, core, outline, &own_libs,
            ),
        ));
        // 3. Duplicate names (`crates/analysis`), per library of the batch.
        for lib in &own_libs {
            let library = program.library(*lib);
            let ids: Vec<UnitId> = library
                .units
                .iter()
                .copied()
                .filter(|u| program.unit(*u).role != dartforge_elements::model::UnitRole::Patch)
                .collect();
            let units: Vec<dartforge_analysis::Unit<'_>> = ids
                .iter()
                .map(|u| dartforge_analysis::Unit {
                    ast: &program.unit(*u).ast,
                    unit: &program.unit(*u).unit,
                    source: &program.unit(*u).source,
                })
                .collect();
            let wildcard = library
                .features
                .has(dartforge_frontend::features::Feature::WildcardVariables);
            // 6.11 merges declarations with the same name; 3.13.4 (the
            // reference of a library with syntax later than 3.6) does not.
            // The library's reference is the highest among its units (T2): the
            // parser marks the unit that uses syntax 3.6.2 does not know, and
            // a version above 3.6 marks all of them.
            let new_syntax =
                program.library_reference(*lib) == dartforge_diagnostics::AnalyzerReference::V3_13;
            let paths: Vec<String> = ids.iter().map(|u| program.unit_path(*u)).collect();
            let mut found: Vec<(usize, Diagnostic, u16)> = with_phase(
                phase::ERROR_VERIFIER,
                dartforge_analysis::duplicates::duplicates_with_paths(
                    &units,
                    &paths,
                    interner,
                    wildcard,
                    !new_syntax,
                ),
            )
            .collect();
            found.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_analysis::enums::without_constants(&units),
            ));
            // `uninitialized_finals` comes after body inference (the type
            // of a field without a type comes from it).

            found.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_analysis::constructors::check(&units, interner),
            ));
            for (i, u) in units.iter().enumerate() {
                let skipped = dartforge_analysis::Skipped {
                    source: u.source,
                    spans: &program.unit(ids[i]).skipped,
                };
                found.extend(with_phase(
                    phase::UNUSED_LOCAL_ELEMENTS,
                    dartforge_analysis::locals::unused_with_skipped(
                        *u, interner, wildcard, skipped,
                    )
                    .into_iter()
                    .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_analysis::externals::initializers(*u)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_analysis::a_context::check(*u)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::BEST_PRACTICES,
                    dartforge_analysis::a_doc::check(*u)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_analysis::c2_syntactic::check(*u, interner)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::UNICODE_TEXT,
                    dartforge_analysis::phases::bidi_text(*u)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::TODO_FINDER,
                    dartforge_analysis::todos::check(u.source)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::LANGUAGE_VERSION,
                    dartforge_analysis::language_version::check(*u)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_analysis::records::check(u, interner, library.features)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_analysis::initialization::uninitialized_constants(u, interner)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::PARSER,
                    dartforge_analysis::phases::colon_for_default_value(
                        *u,
                        library.features.version().major < 3,
                    )
                    .into_iter()
                    .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::RESOLUTION_VISITOR,
                    dartforge_analysis::phases::built_in_as_type(
                        *u,
                        interner,
                        library
                            .features
                            .has(dartforge_frontend::features::Feature::Augmentations)
                            || library
                                .features
                                .has(dartforge_frontend::features::Feature::Macros),
                    )
                    .into_iter()
                    .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::PARSER,
                    dartforge_analysis::natives::outside_sdk(*u)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
                found.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_analysis::operators::arity(*u, interner)
                        .into_iter()
                        .map(|d| (i, d)),
                ));
            }
            // Unused privates: the `UnusedLocalElementsVerifier` runs on every
            // library, with or without syntax errors (`_computeWarnings`,
            // library_analyzer.dart:513-521): the semantic one after
            // inference, and the by-name report kept for when there are no
            // bodies. A name cited in a fragment the parser skipped is not
            // reported (fasta would have read it).
            let skipped: Vec<dartforge_analysis::Skipped<'_>> = ids
                .iter()
                .map(|u| dartforge_analysis::Skipped {
                    source: &program.unit(*u).source,
                    spans: &program.unit(*u).skipped,
                })
                .collect();
            let deferred =
                dartforge_analysis::private_decls::unused_with_skipped(&units, interner, &skipped);
            deferred_privates.insert(
                *lib,
                deferred
                    .into_iter()
                    .map(|(i, d)| (ids[i], d, phase::UNUSED_LOCAL_ELEMENTS))
                    .collect(),
            );
            for (i, d, f) in found {
                if let Some(p) = &program.unit(ids[i]).path {
                    analysis.report(&key(p), d, f);
                }
            }
            for (u, d, f) in with_phase(
                phase::DIRECTIVES,
                dartforge_analysis::imports::deferred_extensions(program, *lib),
            )
            .chain(with_phase(
                phase::DIRECTIVES,
                dartforge_analysis::imports::internal_and_deferred_directives(program, *lib),
            ))
            .chain(with_phase(
                phase::DIRECTIVES,
                dartforge_analysis::imports::undefined_shown_names(program, *lib, interner),
            ))
            .chain(with_phase(
                phase::DIRECTIVES,
                dartforge_analysis::imports::ambiguous_exports(program, *lib, interner),
            ))
            .chain(with_phase(
                phase::RESOLUTION_VISITOR,
                dartforge_analysis::imports::deferred_types(program, *lib, interner),
            ))
            // NEW PHASES (INFRA step 6): the per-library warning verifiers.
            .chain(with_phase(
                phase::IMPORTS_VERIFIER,
                dartforge_analysis::phases::repeated_directives(program, *lib, interner),
            ))
            // The `package:meta` annotations (batch II.7). Public API: the
            // library in `lib/`, outside `lib/src/`.
            .chain({
                let public = program
                    .library(*lib)
                    .units
                    .first()
                    .and_then(|u| program.unit(*u).path.as_deref())
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .is_some_and(|rel| rel.starts_with("lib/") && !rel.starts_with("lib/src/"));
                with_phase(
                    phase::BEST_PRACTICES,
                    dartforge_analysis::meta::check(program, *lib, interner, public),
                )
            }) {
                if let Some(p) = &program.unit(u).path {
                    analysis.report(&key(p), d, f);
                }
            }
            for (u, d, f) in with_phase(
                phase::RESOLVER_VISITOR,
                dartforge_analysis::modifiers::outside_library(program, *lib, interner),
            ) {
                if let Some(p) = &program.unit(u).path {
                    analysis.report(&key(p), d, f);
                }
            }
            for (u, d, f) in with_phase(
                phase::RESOLUTION_VISITOR,
                dartforge_analysis::clauses::check(program, *lib, interner),
            )
            .chain(with_phase(
                phase::MEMBER_DUPLICATE_DEFINITION,
                dartforge_analysis::members::check(program, *lib, interner),
            ))
            .chain(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_analysis::simple_bounds::check(program, *lib, interner, simple_bounds),
            )) {
                if let Some(p) = &program.unit(u).path {
                    analysis.report(&key(p), d, f);
                }
            }
        }

        if cancelled() {
            return None;
        }

        // 4. Types (the outline was already resolved before the clauses).
        times.mark("declaration verifiers");
        let index = Index::new(program, &own_units);
        // Initializers: `types` infers them on every pass, from any library.
        // A pass with no bodies at all (`&[]`) stabilizes the inferred types;
        // the second gives only their diagnostics.
        // A kept program whose signatures and initializers are the same keeps
        // what the passes found (and the inferred types they left).
        let (init_diags, init_units) = match init.as_ref() {
            Some((d, u)) if from_world => (d.clone(), u.clone()),
            _ => {
                let _ = dartforge_types::infer_library_bodies(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    &[],
                );
                let (_, d, u) = dartforge_types::infer_library_bodies_with_units(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    &[],
                    false,
                );
                *init = u
                    .iter()
                    .all(Option::is_some)
                    .then(|| (d.clone(), u.clone()));
                (d, u)
            }
        };
        times.mark("initializer passes");
        let mut attributed: Vec<(UnitId, Diagnostic, u16)> = Vec::new();
        // The libraries on which analyzer 3.6.2 throws (`phase_ffi::AnalyzerCrash`).
        let mut crashed: BTreeSet<dartforge_elements::model::LibraryId> = BTreeSet::new();
        // The side tables of the batch's libraries, each unit from its own
        // library's pass (constant evaluation reads all of them).
        let mut bodies: Option<dartforge_types::BodyTypes> = None;
        // Bodies: one pass per library of the batch, sharing what does not
        // depend on the pass (interfaces, inferred variables, field promotability).
        let shared = dartforge_types::SharedInference::default();
        for lib in &own_libs {
            if cancelled() {
                return None;
            }
            let (bt, ds, us) = dartforge_types::infer_library_bodies_shared(
                program,
                interner,
                table,
                core,
                outline,
                std::slice::from_ref(lib),
                true,
                &shared,
            );
            match &mut bodies {
                None => bodies = Some(bt),
                Some(c) => {
                    let mut bt = bt;
                    for u in &program.library(*lib).units {
                        let i = u.0 as usize;
                        if i < c.units.len() && i < bt.units.len() {
                            c.units[i] = std::mem::take(&mut bt.units[i]);
                        }
                    }
                    // The extensions this pass chose (the `unused_import`'s
                    // `notifyExtensionUsed`).
                    c.used_extensions.extend(bt.used_extensions.drain());
                }
            }
            let start = if ds.len() >= init_diags.len() && ds[..init_diags.len()] == init_diags[..]
            {
                init_diags.len()
            } else {
                // The prefix changed: it cannot be separated; counts as ambiguous.
                analysis.ambiguous += 1;
                0
            };
            let units: Vec<UnitId> = program.library(*lib).units.clone();
            for (d, registered) in ds[start..].iter().zip(&us[start..]) {
                // The unit of the body under inference, when it is from this
                // library; otherwise, by range.
                let u = match registered {
                    Some(u) if units.contains(u) => Some(*u),
                    _ if units.len() == 1 => Some(units[0]),
                    _ => index.attribute_among(program, d.span, &units),
                };
                if let Some(u) = u {
                    attributed.push((u, d.clone(), phase::RESOLVER_VISITOR));
                }
            }
        }
        // Initializers, in order, in the first initializer that contains them.
        times.mark("bodies");
        let inits = initializers(program);
        let mut pos = 0;
        for (d, registered) in init_diags.iter().zip(&init_units) {
            if let Some(u) = registered {
                attributed.push((*u, d.clone(), phase::RESOLVER_VISITOR));
                continue;
            }
            match (pos..inits.len())
                .find(|&i| inits[i].1.start <= d.span.start && d.span.end <= inits[i].1.end)
            {
                Some(i) => {
                    pos = i;
                    attributed.push((inits[i].0, d.clone(), phase::RESOLVER_VISITOR));
                }
                None => {
                    if let Some((u, amb)) = index.attribute(program, d.span) {
                        analysis.ambiguous += usize::from(amb);
                        attributed.push((u, d.clone(), phase::RESOLVER_VISITOR));
                    }
                }
            }
        }
        // Outline: in the unit of the resolved annotation.
        for (d, u) in outline_diags.iter().zip(outline_units) {
            attributed.push((*u, d.clone(), phase::RESOLUTION_VISITOR));
        }
        // The libraries outside the batch whose constants the evaluation reads
        // (cited `const` variables and constructors, transitively): the
        // analyzer resolves the constant initializers of every library at link
        // time (`ConstantInitializersResolver`) and evaluates them on demand
        // (`computeConstantValue`); here they are inferred, reporting nothing
        // of theirs. The SDK stays opaque (a valid unknown value).
        let mut inferred: HashSet<LibraryId> = own_libs.iter().copied().collect();
        if let Some(c) = &mut bodies {
            loop {
                let new = libraries_of_cited_constants(program, c, &inferred);
                if new.is_empty() {
                    break;
                }
                for lib in new {
                    if cancelled() {
                        return None;
                    }
                    let (mut bt, _, _) = dartforge_types::infer_library_bodies_shared(
                        program,
                        interner,
                        table,
                        core,
                        outline,
                        std::slice::from_ref(&lib),
                        true,
                        &shared,
                    );
                    for u in &program.library(lib).units {
                        let i = u.0 as usize;
                        if i < c.units.len() && i < bt.units.len() {
                            c.units[i] = std::mem::take(&mut bt.units[i]);
                        }
                    }
                    inferred.insert(lib);
                }
            }
        }
        // Constants (`ConstantVerifier`), reported only on the batch's.
        times.mark("cited constants");
        if let Some(bodies) = &bodies {
            // The exhaustiveness on which 3.6.2 throws removes the whole library.
            let (reports, crashes) = dartforge_types::const_eval::verify_with_crashes(
                program, interner, table, core, outline, bodies, &inferred, &own_libs,
            );
            attributed.extend(with_phase(phase::CONSTANT_VERIFIER, reports));
            crashed.extend(crashes);
            // `BestPracticesVerifier`: `non_const_call_to_literal_constructor`,
            // with `canBeConst` through the constant verification.
            attributed.extend(with_phase(
                phase::BEST_PRACTICES,
                dartforge_types::phase_literal::literal_constructors(
                    program, interner, table, core, outline, bodies, &inferred, &own_libs,
                ),
            ));
        }
        // `_checkForFinalNotInitializedInClass` and the `ConstructorFieldsVerifier`
        times.mark("constant verification");
        // (`crates/analysis`), with the `isPotentiallyNonNullable` of the type of
        // each instance field (the written or the inferred one, already with
        // the bodies).
        for lib in &own_libs {
            let ids: Vec<UnitId> = program
                .library(*lib)
                .units
                .iter()
                .copied()
                .filter(|u| program.unit(*u).role != dartforge_elements::model::UnitRole::Patch)
                .collect();
            let units: Vec<dartforge_analysis::Unit<'_>> = ids
                .iter()
                .map(|u| dartforge_analysis::Unit {
                    ast: &program.unit(*u).ast,
                    unit: &program.unit(*u).unit,
                    source: &program.unit(*u).source,
                })
                .collect();
            let mut non_nullable: std::collections::HashMap<(usize, usize), bool> =
                std::collections::HashMap::new();
            for (vi, var) in program.variables.iter().enumerate() {
                if var.library != *lib || var.static_ {
                    continue;
                }
                let dartforge_elements::model::VariableRef::Field {
                    unit,
                    member,
                    index,
                } = var.node
                else {
                    continue;
                };
                let Some(ui) = ids.iter().position(|u| *u == unit) else {
                    continue;
                };
                let dartforge_frontend::ast::MemberKind::Field(vl) =
                    &program.unit(unit).ast.member(member).kind
                else {
                    continue;
                };
                let Some(v) = vl.variables.get(index) else {
                    continue;
                };
                let od = &outline.variables[vi];
                let Some(t) = od.declared_type.or(od.inferred) else {
                    continue;
                };
                let mut env =
                    dartforge_types::subtyping::SubtypeEnv::new(table, &outline.hierarchy, core);
                let nullable = dartforge_types::subtyping::is_subtype(core.null, t, &mut env);
                non_nullable.insert((ui, v.name.span.start), !nullable);
            }
            let found = dartforge_analysis::initialization::uninitialized_finals_with(
                &units,
                interner,
                &|u, s| non_nullable.get(&(u, s)).copied(),
            );
            // An extension type without a primary constructor: 3.6.2 reports the
            // synthetic representation (`'<empty>'`); 3.13.4 has no such field.
            let found = found.into_iter().filter(|(i, d)| {
                !(program.reference(ids[*i]) == dartforge_diagnostics::AnalyzerReference::V3_13
                    && d.code == Some(dartforge_diagnostics::codes::compile_time_error::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1)
                    && d.message.contains("'<empty>'"))
            });
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                found.into_iter().map(|(i, d)| (ids[i], d)),
            ));
        }
        // Invalid overrides, in the classes where the `verify()` of the
        // `InheritanceOverrideVerifier` gets to check them.
        for lib in &own_libs {
            let classes =
                dartforge_analysis::clauses::inheritance_verifier_proceeds(program, *lib, interner);
            // `verify()`: the interface conflicts before the members.
            attributed.extend(with_phase(
                phase::INHERITANCE_OVERRIDE,
                dartforge_types::phase_inheritance::inconsistencies(
                    program, interner, table, core, outline, &classes,
                ),
            ));
            attributed.extend(with_phase(
                phase::INHERITANCE_OVERRIDE,
                dartforge_types::override_checks::invalid_overrides(
                    program, interner, table, core, outline, &classes,
                ),
            ));
            attributed.extend(with_phase(
                phase::INHERITANCE_OVERRIDE,
                dartforge_types::override_checks::enum_members(
                    program, interner, table, core, outline, &classes,
                ),
            ));
            attributed.extend(with_phase(
                phase::INHERITANCE_OVERRIDE,
                dartforge_types::override_checks::abstract_members(
                    program, interner, table, core, outline, &classes,
                ),
            ));
            attributed.extend(with_phase(
                phase::INHERITANCE_OVERRIDE,
                dartforge_types::override_checks::conflicting_members(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::override_checks::enum_statics(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::INHERITANCE_OVERRIDE,
                dartforge_types::override_checks::default_values_with(
                    program,
                    interner,
                    table,
                    Some(core),
                    outline,
                    bodies.as_ref(),
                    *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::override_checks::uninitialized_variables(
                    program, interner, table, outline, *lib,
                ),
            ));
            // `BestPracticesVerifier` over declarations and written types.
            attributed.extend(with_phase(
                phase::BEST_PRACTICES,
                dartforge_types::best_practices::nullable_equality_parameter(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::BEST_PRACTICES,
                dartforge_types::best_practices::unnecessary_no_such_method(
                    program, interner, core, *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::BEST_PRACTICES,
                dartforge_types::best_practices::unnecessary_question_marks(
                    program,
                    interner,
                    table,
                    outline,
                    bodies.as_ref(),
                    *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::override_checks::getters_and_setters(
                    program, interner, table, core, outline, *lib, &classes,
                ),
            ));
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::variance::variance(program, interner, table, outline, *lib),
            ));
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::variance::non_covariant_positions_in_representation(
                    program, table, outline, *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::a_main::main_function(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            // `conflicting_generic_interfaces`, with the clauses' gate.
            {
                let open = |id: dartforge_elements::model::ClassId| {
                    dartforge_analysis::clauses::gate(program, *lib, interner, id)
                        == dartforge_analysis::clauses::Gate::Open
                };
                attributed.extend(with_phase(
                    phase::ERROR_VERIFIER,
                    dartforge_types::phase_generics::generic_conflicts(
                        program, interner, table, core, outline, *lib, &open,
                    ),
                ));
            }
            // `visitEnumDeclaration`: the enum instantiated to bounds.
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::bound_checks::enum_instantiated_to_bounds(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            // `extension type` declarations: cycles, bottom, conflicts and `implements`.
            attributed.extend(with_phase(
                phase::ERROR_VERIFIER,
                dartforge_types::extension_types::verify(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            // NEW PHASES (INFRA step 6): `OverrideVerifier`.
            attributed.extend(with_phase(
                phase::OVERRIDE_VERIFIER,
                dartforge_types::phase_override::missing_override(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            attributed.extend(with_phase(
                phase::REDECLARE_VERIFIER,
                dartforge_types::phase_override::missing_redeclare(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            // `BestPracticesVerifier`: `invalid_override_of_non_virtual_member`.
            attributed.extend(with_phase(
                phase::BEST_PRACTICES,
                dartforge_types::phase_override::non_virtual_overrides(
                    program, interner, table, core, outline, *lib,
                ),
            ));
            // The `annotate_overrides` lint, kept apart: it only comes out with
            // the rule enabled.
            for (u, span, name) in dartforge_types::phase_override::missing_override_annotation(
                program, interner, table, core, outline, *lib,
            ) {
                if let Some(p) = &program.unit(u).path
                    && let Some(a) = analysis.files.get_mut(&key(p))
                {
                    a.semantic_lints.push(SemanticLint {
                        unique: "annotate_overrides",
                        span,
                        args: vec![name],
                    });
                }
            }
            // `MustCallSuperVerifier`.
            attributed.extend(with_phase(
                phase::BEST_PRACTICES,
                dartforge_types::phase_super::missing_super_call(program, interner, *lib),
            ));
            if let Some(bodies) = &bodies {
                // Also runs on the libraries judged by 3.13.4: there the code
                // comes out as `unused_element_parameter`, through the variant
                // (`Diagnostic::for_reference`, in the final loop).
                attributed.extend(with_phase(
                    phase::UNUSED_LOCAL_ELEMENTS,
                    dartforge_types::parameters::unused_parameters(
                        program, interner, outline, bodies, *lib,
                    ),
                ));
            }
            // `UnusedLocalElementsVerifier` (library declarations).
            if let Some(deferred) = deferred_privates.remove(lib) {
                match &bodies {
                    Some(bodies) => {
                        let inferred: HashSet<LibraryId> = own_libs.iter().copied().collect();
                        let mut unused = dartforge_types::phase_unused::unused_elements(
                            program, interner, table, core, outline, bodies, &inferred, *lib,
                        );
                        let units = &program.library(*lib).units;
                        unused.retain(|(u, d)| {
                            let name = program
                                .unit(*u)
                                .source
                                .get(d.span.start..d.span.end)
                                .unwrap_or("");
                            !units.iter().any(|x| {
                                dartforge_analysis::Skipped {
                                    source: &program.unit(*x).source,
                                    spans: &program.unit(*x).skipped,
                                }
                                .mentions(name)
                            })
                        });
                        attributed.extend(with_phase(phase::UNUSED_LOCAL_ELEMENTS, unused));
                    }
                    None => attributed.extend(deferred),
                }
            }
        }
        // `SdkConstraintVerifier`: only with a readable `environment: sdk:` in
        // the root's `pubspec.yaml`.
        let sdk_constraint = std::fs::read_to_string(root.join("pubspec.yaml"))
            .ok()
            .and_then(|t| yaml_rust2::YamlLoader::load_from_str(&t).ok())
            .and_then(|docs| docs.into_iter().next())
            .and_then(|y| {
                y["environment"]["sdk"]
                    .as_str()
                    .and_then(dartforge_types::phase_sdk::SdkConstraint::from_text)
            });
        // Type arguments out of bounds, unit by unit.
        // The inferred libraries, for the lints' constant engine.
        let lint_inferred: HashSet<LibraryId> = own_libs.iter().copied().collect();
        for &u in &own_units {
            let unit_body = bodies.as_ref().and_then(|c| c.units.get(u.0 as usize));
            for d in dartforge_types::bound_checks::arguments_out_of_bounds(
                program, interner, table, core, outline, unit_body, u,
            ) {
                attributed.push((u, d, phase::ERROR_VERIFIER));
            }
            // `DeprecatedMemberUseVerifier`: "same package" is the library
            // whose file lies inside the analyzed root.
            let same_package = |l: dartforge_elements::model::LibraryId| {
                program
                    .library(l)
                    .units
                    .first()
                    .and_then(|x| program.unit(*x).path.as_deref())
                    .is_some_and(|p| p.starts_with(root))
            };
            let unit_body = bodies.as_ref().and_then(|c| c.units.get(u.0 as usize));
            for d in dartforge_types::phase_deprecated::deprecated_uses(
                program,
                interner,
                table,
                unit_body,
                u,
                &same_package,
            ) {
                attributed.push((u, d, phase::BEST_PRACTICES));
            }
            // 3.13.4's `DeprecatedFunctionalityVerifier` (in the
            // `BestPracticesVerifier`): `DEPRECATED_OPTIONAL`.
            for d in dartforge_types::phase_deprecated::deprecated_optionals(
                program, interner, unit_body, u,
            ) {
                attributed.push((u, d, phase::BEST_PRACTICES));
            }
            // `_InvalidAccessVerifier` and the `@sealed` classes.
            for d in dartforge_types::phase_access::invalid_accesses(
                program,
                interner,
                outline,
                unit_body,
                u,
                &same_package,
            ) {
                attributed.push((u, d, phase::BEST_PRACTICES));
            }
            if let Some(constraint) = &sdk_constraint {
                for d in dartforge_types::phase_sdk::sdk_constraint(
                    program, interner, unit_body, u, constraint,
                ) {
                    attributed.push((u, d, phase::SDK_CONSTRAINT));
                }
            }
            if let Some(all) = bodies.as_ref()
                && let Some(body) = all.units.get(u.0 as usize)
            {
                // The typed lint rules, kept apart: they only come out with the
                // rule enabled.
                let mut from_lints =
                    dartforge_types::typed_lints::findings(program, table, core, outline, body, u);
                from_lints.extend(dartforge_types::typed_lints2::findings(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    all,
                    &lint_inferred,
                    u,
                ));
                from_lints.extend(dartforge_types::typed_lints3::findings(
                    program, interner, table, core, outline, body, u,
                ));
                from_lints.extend(dartforge_types::typed_lints4::findings(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    all,
                    &lint_inferred,
                    u,
                ));
                from_lints.extend(dartforge_types::typed_lints5::findings(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    all,
                    &lint_inferred,
                    u,
                ));
                from_lints.extend(dartforge_types::typed_lints7::findings(
                    program, table, core, outline, body, u,
                ));
                from_lints.extend(dartforge_types::typed_lints8::findings(
                    program, table, core, outline, body, u,
                ));
                from_lints.extend(dartforge_types::typed_lints9::findings(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    all,
                    &lint_inferred,
                    u,
                ));
                from_lints.extend(dartforge_types::phase_deprecated::same_package_lint(
                    program,
                    interner,
                    table,
                    Some(body),
                    u,
                    &same_package,
                ));
                if !from_lints.is_empty()
                    && let Some(p) = &program.unit(u).path
                    && let Some(a) = analysis.files.get_mut(&key(p))
                {
                    a.semantic_lints.extend(
                        from_lints
                            .into_iter()
                            .map(|(span, unique, args)| SemanticLint { unique, span, args }),
                    );
                }
                // The tree lint rules, over the program's tree and with the
                // engine's semantics.
                {
                    let can_be_const = dartforge_types::typed_lints6::can_be_const(
                        program,
                        interner,
                        table,
                        core,
                        outline,
                        all,
                        &lint_inferred,
                        u,
                    );
                    let un = program.unit(u);
                    let sem = dartforge_analysis::lints::Semantics {
                        program,
                        unit: u,
                        body,
                        bodies: all,
                        table,
                        core,
                        outline,
                        can_be_const: Some(&can_be_const),
                    };
                    let tree = dartforge_analysis::Unit {
                        ast: &un.ast,
                        unit: &un.unit,
                        source: &un.source,
                    };
                    let reports =
                        dartforge_analysis::lints::run_with(tree, interner, &|_| true, Some(&sem));
                    if let Some(p) = &un.path
                        && let Some(a) = analysis.files.get_mut(&key(p))
                    {
                        a.lint_reports = Some(reports);
                    }
                }
                // `RequiredParametersVerifier`: `package:meta`'s `@required`.
                for d in
                    dartforge_types::phase_required::missing_required(program, interner, body, u)
                {
                    attributed.push((u, d, phase::ERROR_VERIFIER));
                }
                // `ErrorHandlerVerifier`: the return of `catchError`'s `onError`.
                for d in dartforge_types::phase_catch_error::catch_error_returns(
                    program, interner, table, core, outline, body, u,
                ) {
                    attributed.push((u, d, phase::BEST_PRACTICES));
                }
                // `FfiVerifier`; the 3.6.2 crash removes the whole library.
                match dartforge_types::phase_ffi::verify(
                    program,
                    interner,
                    table,
                    core,
                    outline,
                    all,
                    &lint_inferred,
                    u,
                ) {
                    Ok(ds) => {
                        for d in ds {
                            attributed.push((u, d, phase::FFI_VERIFIER));
                        }
                    }
                    Err(dartforge_types::phase_ffi::AnalyzerCrash) => {
                        crashed.insert(program.unit(u).library);
                    }
                }
                // `BestPracticesVerifier`: `assignment_of_do_not_store` and `return_of_do_not_store`.
                for d in dartforge_types::phase_do_not_store::stored_and_returned(
                    program, interner, body, u,
                ) {
                    attributed.push((u, d, phase::BEST_PRACTICES));
                }
                // `BestPracticesVerifier` with `strict-inference` (filtered out without the option).
                for d in dartforge_types::phase_strict::inference_failures(
                    program, interner, table, core, outline, body, u,
                ) {
                    attributed.push((u, d, phase::BEST_PRACTICES));
                }
                // `UseResultVerifier`.
                for d in dartforge_types::phase_result::unused_results(program, interner, body, u) {
                    attributed.push((u, d, phase::ERROR_VERIFIER));
                }
                for d in dartforge_types::bound_checks::inferred_arguments_out_of_bounds(
                    program, interner, table, core, outline, body, u,
                ) {
                    attributed.push((u, d, phase::ERROR_VERIFIER));
                }
            }
        }
        // In the types of `extends`/`implements`/`with` the analyzer reports
        // neither an undefined name nor a name that is not a type: the error
        // is the `*_non_class` of `analysis::clausulas`.
        let clause_names: BTreeSet<(UnitId, usize)> = own_libs
            .iter()
            .flat_map(|lib| dartforge_analysis::clauses::clause_name_starts(program, *lib))
            .collect();
        let mut seen: BTreeSet<(
            UnitId,
            Option<dartforge_diagnostics::Code>,
            usize,
            usize,
            String,
        )> = BTreeSet::new();
        for (unit, d, emitter_phase) in &attributed {
            let unit = *unit;
            if !own_units.contains(&unit) {
                continue;
            }
            let source = &program.unit(unit).source;
            let fragment = source.get(d.span.start..d.span.end).unwrap_or("");
            // The constant codes come from `types::const_eval`; what inference
            // emits of them (without a code) is left out.
            if d.code.is_none() {
                let coded = bridge::encode_types(d, fragment);
                if coded
                    .code
                    .is_some_and(|c| dartforge_types::const_eval::CODES.contains(&c.info().name))
                {
                    continue;
                }
            }
            let coded = bridge::encode_types(d, fragment);
            // An annotation's validity depends on the SDK
            // (`@Deprecated.optional()` only exists after 3.6.2): in a library
            // judged by 3.13.4, our 3.6.2 SDK does not decide.
            if coded.code.is_some_and(|c| {
                matches!(c.info().name, "invalid_annotation" | "undefined_annotation")
            }) && program.library_reference(program.unit(unit).library)
                == dartforge_diagnostics::AnalyzerReference::V3_13
            {
                continue;
            }
            if coded
                .code
                .is_some_and(|c| matches!(c.info().name, "undefined_class" | "not_a_type"))
                && clause_names.contains(&(unit, coded.span.start))
            {
                continue;
            }
            if !seen.insert((
                unit,
                coded.code,
                coded.span.start,
                coded.span.end,
                coded.message.clone(),
            )) {
                continue;
            }
            let k = key(program
                .unit(unit)
                .path
                .as_deref()
                .expect("an own unit has a path"));
            analysis.report(&k, coded, *emitter_phase);
        }

        // 5. Unused imports, after everything (the suppression looks at the
        times.mark("type verifiers");
        // library's diagnostics).
        for lib in &own_libs {
            let keys: Vec<PathBuf> = program
                .library(*lib)
                .units
                .iter()
                .filter_map(|u| program.unit(*u).path.as_deref().map(key))
                .collect();
            let existing: Vec<Diagnostic> = keys
                .iter()
                .filter_map(|k| analysis.files.get(k))
                .flat_map(|a| a.diags.iter().cloned())
                .collect();
            for (u, d) in dartforge_analysis::imports::unused(
                program,
                *lib,
                interner,
                &existing,
                bodies.as_ref(),
            ) {
                if let Some(p) = &program.unit(u).path {
                    analysis.report(&key(p), d, phase::IMPORTS_VERIFIER);
                }
            }
        }

        // The emission order (INFRA III.5 item 3, §1.2): each file's list comes
        // out in the order of the `LibraryAnalyzer`'s phases; within a phase,
        // in the order the emitter reported. It decides which of two equal
        // reports survives (the final deduplication keeps the first) and the
        // order the LSP publishes.
        analysis.sort_by_phases();

        // 6. The skipped-fragments rule (T5, structure (c)): "not defined" for
        // a name cited in a skipped fragment of the library; and the two
        // declaration-shape codes, when the range touches a skipped fragment.
        {
            let mut library_units: HashMap<PathBuf, Vec<UnitId>> = HashMap::new();
            for lib in &own_libs {
                let units: Vec<UnitId> = program.library(*lib).units.clone();
                for u in &units {
                    if let Some(p) = &program.unit(*u).path {
                        library_units.insert(key(p), units.clone());
                    }
                }
            }
            for (k, a) in analysis.files.iter_mut() {
                let Some(units) = library_units.get(k) else {
                    continue;
                };
                let Some(&own_unit) = unit_of.get(k) else {
                    continue;
                };
                let source = program.unit(own_unit).source.as_str();
                let own_skipped = dartforge_analysis::Skipped {
                    source,
                    spans: &program.unit(own_unit).skipped,
                };
                let n = a.syntactic_count;
                let mut i = 0;
                a.diags.retain(|d| {
                    i += 1;
                    if i <= n {
                        return true;
                    }
                    let Some(c) = d.code.map(|c| c.info().name) else {
                        return true;
                    };
                    if depends_on_declarations(c) {
                        let name = source.get(d.span.start..d.span.end).unwrap_or("");
                        return !units.iter().any(|u| {
                            dartforge_analysis::Skipped {
                                source: &program.unit(*u).source,
                                spans: &program.unit(*u).skipped,
                            }
                            .mentions(name)
                        });
                    }
                    if depends_on_line(c) {
                        return !own_skipped.intersects(d.span);
                    }
                    true
                });
            }
        }
        // The analyzer does not report the same diagnostic twice (code, range
        // and message) — the `_checkDuplicateIdentifier` of a repeated mutable
        // variable asks for the same error through the getter and the setter,
        // and the result has only one.
        for (k, a) in analysis.files.iter_mut() {
            // T2's single point: each diagnostic comes out with the name, text
            // and arguments of the analyzer that is the file's reference. No
            // emitter consults the reference; they just pass, after the 3.6
            // template's arguments, those only 3.13 uses.
            let reference = unit_of
                .get(k)
                .map_or(dartforge_diagnostics::AnalyzerReference::V3_6, |u| {
                    program.reference(*u)
                });
            if reference == dartforge_diagnostics::AnalyzerReference::V3_13 {
                let n = a.syntactic_count;
                let mut syntactic_dropped = 0;
                let before = std::mem::take(&mut a.diags);
                for (i, d) in before.into_iter().enumerate() {
                    match d.for_reference(reference) {
                        Some(d) => a.diags.push(d),
                        None if i < n => syntactic_dropped += 1,
                        None => {}
                    }
                }
                a.syntactic_count -= syntactic_dropped;
            }
            // The synthetic identifier the parser inserts (`x = ;`, `a.`) has
            // an empty name and is not resolved: the analyzer's resolver
            // reports no undefined name on it.
            {
                let n = a.syntactic_count;
                let mut i = 0;
                a.diags.retain(|d| {
                    i += 1;
                    i <= n
                        || d.span.start != d.span.end
                        || !d
                            .code
                            .is_some_and(|c| c.info().name.starts_with("undefined_"))
                });
            }
            let mut seen: BTreeSet<(Option<dartforge_diagnostics::Code>, usize, usize, String)> =
                BTreeSet::new();
            let n = a.syntactic_count;
            let mut i = 0;
            let mut syntactic_removed = 0;
            a.diags.retain(|d| {
                i += 1;
                let new = seen.insert((d.code, d.span.start, d.span.end, d.message.clone()));
                if !new && i <= n {
                    syntactic_removed += 1;
                }
                new
            });
            a.syntactic_count -= syntactic_removed;
        }
        // The library on which the analyzer throws gets no result: no
        // diagnostic (neither syntactic nor lints) of its files.
        times.mark("unused imports, lints");
        for lib in &crashed {
            for u in &program.library(*lib).units {
                if let Some(p) = &program.unit(*u).path
                    && let Some(a) = analysis.files.get_mut(&key(p))
                {
                    a.diags.clear();
                    a.syntactic_count = 0;
                    a.semantic_lints.clear();
                    a.lint_reports = None;
                }
            }
        }
        Some(analysis)
    }

    /// The target of a `part` exists and has no `part of` (its `kind` is not
    /// `PartFileKind`): its `uriStr` (that of the loaded unit, otherwise
    /// `dart:`/`package:` as written, otherwise `file:///…`).
    fn part_without_part_of(
        &self,
        uri: &str,
        base: &Path,
        config: Option<&dartforge_elements::PackageConfig>,
        program: &dartforge_elements::model::Program,
        texts: &HashMap<std::path::PathBuf, String>,
    ) -> Option<String> {
        let path = if let Some(name) = uri.strip_prefix("dart:") {
            match name.split_once('/') {
                Some((lib, rest)) => self.sdk.libraries.get(lib)?.path.parent()?.join(rest),
                None => self.sdk.libraries.get(name)?.path.clone(),
            }
        } else if uri.starts_with("package:") {
            config?.resolve_package_uri(uri).ok()?
        } else if uri.contains(':') {
            return None;
        } else {
            base.join(uri)
        };
        let k = key(&path);
        let loaded = program
            .units
            .iter()
            .find(|u| u.path.as_deref().map(key).as_ref() == Some(&k));
        let has_part_of = |directives: &[dartforge_frontend::ast::Directive]| {
            directives
                .iter()
                .any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))
        };
        let part = match (texts.get(&k), loaded) {
            (Some(text), _) => {
                let mut names = dartforge_intern::Interner::new();
                has_part_of(
                    &dartforge_frontend::parser::parse(text, &mut names)
                        .unit
                        .directives,
                )
            }
            (None, Some(u)) => has_part_of(&u.unit.directives),
            (None, None) => {
                let text = std::fs::read_to_string(&path).ok()?;
                let mut names = dartforge_intern::Interner::new();
                has_part_of(
                    &dartforge_frontend::parser::parse(&text, &mut names)
                        .unit
                        .directives,
                )
            }
        };
        if part {
            return None;
        }
        if let Some(u) = loaded
            && !u.uri.is_empty()
        {
            return Some(u.uri.clone());
        }
        if uri.contains(':') {
            return Some(uri.to_string());
        }
        let abs = k.to_string_lossy().replace('\\', "/");
        Some(format!(
            "file://{}{abs}",
            if abs.starts_with('/') { "" } else { "/" }
        ))
    }

    /// `URI_DOES_NOT_EXIST` / `URI_HAS_NOT_BEEN_GENERATED`
    /// (`library_analyzer.dart:670-720`): the directive's target does not exist.
    fn directive_without_target(
        &self,
        uri: &str,
        base: &Path,
        config: Option<&dartforge_elements::PackageConfig>,
        span: Span,
        part: bool,
    ) -> Option<Diagnostic> {
        if uri.is_empty() {
            // `import ''` resolves to the library itself, which exists.
            return None;
        }
        let exists = if let Some(name) = uri.strip_prefix("dart:") {
            match name.split_once('/') {
                // `dart:async/future.dart`: a file next to the `dart:async` library.
                Some((lib, rest)) => self
                    .sdk
                    .libraries
                    .get(lib)
                    .and_then(|l| l.path.parent())
                    .is_some_and(|d| d.join(rest).is_file()),
                None => self.sdk_libraries.contains(name),
            }
        } else if uri.starts_with("package:") {
            match config.map(|c| c.resolve_package_uri(uri)) {
                Some(Ok(p)) => p.is_file(),
                _ => false,
            }
        } else if uri.contains(':') {
            // Other schemes (`dart-ext:`, `http:`): out of scope.
            return None;
        } else {
            let target =
                key(&resolve_directive_uri(uri, base, config).unwrap_or_else(|| base.join(uri)));
            target.is_file() || config.is_some_and(|c| generated_by_build(c, &target))
        };
        if exists {
            return None;
        }
        let code = if !uri.starts_with("dart:") && is_generated(uri) {
            codes::compile_time_error::URI_HAS_NOT_BEEN_GENERATED
        } else {
            codes::compile_time_error::URI_DOES_NOT_EXIST
        };
        // In a `part` with a relative URI, the analyzer shows the resolved URI
        // (`file:///…/a.dart`); in `import`/`export`, the directive's text.
        if part && !uri.contains(':') {
            let abs = key(&base.join(uri)).to_string_lossy().replace('\\', "/");
            let file_uri = format!(
                "file://{}{abs}",
                if abs.starts_with('/') { "" } else { "/" }
            );
            return Some(Diagnostic::with_code(code, span, [file_uri.as_str()]));
        }
        Some(Diagnostic::with_code(code, span, [uri]))
    }
}

/// Codes of a name that does not resolve in an **expression**: they depend on
/// every declaration of the library having been recovered by the parser, and
/// our recovery of statements and expressions still diverges from fasta's in
/// measured cases (FP of `undefined_identifier`/`undefined_getter` without
/// this gate). **Type** names (`undefined_class`, `not_a_type` and those of
/// the `NamedTypeResolver`) do not go through here: the recovery of
/// declarations is fasta's (docs/ANALISADOR-PARIDADE-PLANO.md §3.2), and the
/// analyzer resolves the recovered tree like any other.
fn depends_on_declarations(code: &str) -> bool {
    matches!(
        code,
        "undefined_identifier"
            | "undefined_function"
            | "undefined_method"
            | "undefined_getter"
            | "undefined_setter"
            | "undefined_operator"
    )
}

/// Codes that depend on the exact shape of a declaration (getter and setter
/// types; a variable without an initializer, which recovery may invent from a
/// getter with an invalid name): they do not come out of a line where the
/// parser recovered.
fn depends_on_line(code: &str) -> bool {
    matches!(
        code,
        "getter_not_subtype_setter_types" | "not_initialized_non_nullable_variable"
    )
}

/// The libraries outside `inferred` (and the SDK) of the `const` variables and
/// `const` constructors that the expressions of the inferred ones cite.
fn libraries_of_cited_constants(
    program: &Program,
    bodies: &dartforge_types::BodyTypes,
    inferred: &HashSet<LibraryId>,
) -> Vec<LibraryId> {
    use dartforge_elements::model::Element;
    use dartforge_types::{MemberRef, Resolved};
    let mut new = BTreeSet::new();
    for &lib in inferred {
        for &u in &program.library(lib).units {
            let Some(b) = bodies.units.get(u.0 as usize) else {
                continue;
            };
            for i in 0..program.unit(u).ast.exprs.len() {
                let e = dartforge_frontend::ast::ExprId(i as u32);
                let target = match b.get_resolved(e) {
                    Some(Resolved::Element(Element::Variable(v)))
                    | Some(Resolved::Member {
                        member: MemberRef::Variable(v),
                        ..
                    }) => {
                        let var = program.variable(*v);
                        var.const_.then_some(var.library)
                    }
                    Some(Resolved::Element(Element::Function(f)))
                    | Some(Resolved::Member {
                        member: MemberRef::Function(f),
                        ..
                    }) => match program.function(*f).variable {
                        Some(v) if program.variable(v).const_ => Some(program.variable(v).library),
                        _ => None,
                    },
                    Some(Resolved::Constructor(f)) => {
                        let fe = program.function(*f);
                        fe.const_.then_some(fe.library)
                    }
                    _ => None,
                };
                if let Some(l) = target
                    && !inferred.contains(&l)
                    && !program.library(l).uri.starts_with("dart:")
                {
                    new.insert(l.0);
                }
            }
        }
    }
    new.into_iter().map(LibraryId).collect()
}

/// `(unit, initializer range)` of the variables outside the SDK, in the order
/// of `program.variables` (the order in which `types` infers them).
fn initializers(program: &Program) -> Vec<(UnitId, Span)> {
    use dartforge_elements::model::VariableRef;
    use dartforge_frontend::ast::{DeclKind, MemberKind};
    let mut v = Vec::new();
    for var in &program.variables {
        if program.library(var.library).is_sdk {
            continue;
        }
        let (unit, init) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => {
                match &program.unit(unit).ast.decls[decl.0 as usize].kind {
                    DeclKind::Variables(l) => {
                        (unit, l.variables.get(index).and_then(|x| x.initializer))
                    }
                    _ => continue,
                }
            }
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &program.unit(unit).ast.members[member.0 as usize].kind {
                MemberKind::Field(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => continue,
            },
            _ => continue,
        };
        if let Some(e) = init {
            v.push((unit, program.unit(unit).ast.exprs[e.0 as usize].span));
        }
    }
    v
}

/// Node ranges per unit, to attribute diagnostics without a unit.
///
/// Only the units of the batch's libraries are kept in a map (they are the ones
/// a diagnostic can be reported in); the other units of the program (the
/// dependencies, thousands of libraries in a workspace package) only need to
/// answer "which units have a node with exactly this range", so they go into
/// one sorted vector of `(start, end, unit)`: 12 bytes per node instead of a
/// map entry with its own vector (on `new_sali`'s frontend, ~100 MiB of the
/// analysis peak). That vector is only built when a diagnostic needs it (one
/// without a unit, outside every initializer), which most analyses never have.
struct Index {
    exact: HashMap<(usize, usize), Vec<UnitId>>,
    /// The libraries whose units are in `exact`.
    own_libraries: BTreeSet<LibraryId>,
    /// The `(start, end, unit)` of the nodes of the other non-SDK units, sorted,
    /// one per range and unit; built on the first query.
    others: std::cell::OnceCell<Vec<(u32, u32, u32)>>,
    /// The ranges of the top-level declarations of the batch's units.
    top: Vec<(UnitId, Span)>,
    own: BTreeSet<UnitId>,
}

impl Index {
    fn new(program: &Program, own: &BTreeSet<UnitId>) -> Index {
        let own_libraries: BTreeSet<LibraryId> =
            own.iter().map(|u| program.unit(*u).library).collect();
        let mut exact: HashMap<(usize, usize), Vec<UnitId>> = HashMap::new();
        let mut top = Vec::new();
        for (i, u) in program.units.iter().enumerate() {
            if program.library(u.library).is_sdk || !Self::in_map(&own_libraries, u) {
                continue;
            }
            let id = UnitId(i as u32);
            for s in Self::spans(u) {
                let v = exact.entry((s.start, s.end)).or_default();
                if v.last() != Some(&id) {
                    v.push(id);
                }
            }
            if own.contains(&id) {
                for d in &u.unit.declarations {
                    top.push((id, u.ast.decls[d.0 as usize].span));
                }
            }
        }
        Index {
            exact,
            own_libraries,
            others: std::cell::OnceCell::new(),
            top,
            own: own.clone(),
        }
    }

    /// The unit goes into the map: it is of the batch's libraries, or a range of
    /// it does not fit in 32 bits.
    fn in_map(own_libraries: &BTreeSet<LibraryId>, u: &dartforge_elements::model::Unit) -> bool {
        own_libraries.contains(&u.library) || u.source.len() > u32::MAX as usize
    }

    /// The ranges of a unit's expressions, statements, types and patterns.
    fn spans(u: &dartforge_elements::model::Unit) -> impl Iterator<Item = Span> + '_ {
        u.ast
            .exprs
            .iter()
            .map(|e| e.span)
            .chain(u.ast.stmts.iter().map(|e| e.span))
            .chain(u.ast.types.iter().map(|e| e.span))
            .chain(u.ast.patterns.iter().map(|e| e.span))
    }

    /// The other units' ranges, built on the first call.
    fn others(&self, program: &Program) -> &[(u32, u32, u32)] {
        self.others.get_or_init(|| {
            let mut others: Vec<(u32, u32, u32)> = Vec::new();
            for (i, u) in program.units.iter().enumerate() {
                if program.library(u.library).is_sdk || Self::in_map(&self.own_libraries, u) {
                    continue;
                }
                others.extend(Self::spans(u).map(|s| (s.start as u32, s.end as u32, i as u32)));
            }
            // One per range and unit (the map's lists hold each unit once).
            others.sort_unstable();
            others.dedup();
            others.shrink_to_fit();
            others
        })
    }

    /// The units with a node of exactly range `s`, in unit order (as the
    /// map's lists): those of the batch's libraries and the others.
    fn with_range(&self, program: &Program, s: Span) -> Vec<UnitId> {
        let mut v: Vec<UnitId> = self
            .exact
            .get(&(s.start, s.end))
            .cloned()
            .unwrap_or_default();
        if let (Ok(start), Ok(end)) = (u32::try_from(s.start), u32::try_from(s.end)) {
            let others = self.others(program);
            let from = others.partition_point(|&(a, b, _)| (a, b) < (start, end));
            v.extend(
                others[from..]
                    .iter()
                    .take_while(|&&(a, b, _)| (a, b) == (start, end))
                    .map(|&(_, _, u)| UnitId(u)),
            );
        }
        v.sort_unstable();
        v
    }

    /// Among the given `units` (those of one library): that of the node with
    /// the exact range; otherwise the first whose top-level declaration
    /// contains it.
    fn attribute_among(&self, program: &Program, s: Span, units: &[UnitId]) -> Option<UnitId> {
        if let Some(v) = self.exact.get(&(s.start, s.end))
            && let Some(u) = v.iter().find(|u| units.contains(u))
        {
            return Some(*u);
        }
        units.iter().copied().find(|u| {
            let unit = program.unit(*u);
            unit.unit.declarations.iter().any(|d| {
                let sp = unit.ast.decls[d.0 as usize].span;
                sp.start <= s.start && s.end <= sp.end
            })
        })
    }

    /// The diagnostic's unit and whether there was more than one candidate.
    fn attribute(&self, program: &Program, s: Span) -> Option<(UnitId, bool)> {
        let v = self.with_range(program, s);
        if !v.is_empty() {
            let chosen = v
                .iter()
                .copied()
                .find(|u| self.own.contains(u))
                .unwrap_or(v[0]);
            return Some((chosen, v.len() > 1));
        }
        let mut cands = self
            .top
            .iter()
            .filter(|(_, d)| d.start <= s.start && s.end <= d.end)
            .map(|(u, _)| *u);
        let first = cands.next()?;
        let other = cands.any(|u| u != first);
        Some((first, other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_by_first_directive() {
        assert!(is_part("// c\n/* x\n y */\npart of 'a.dart';\n"));
        assert!(!is_part("library a;\npart 'b.dart';\n"));
        assert!(!is_part("void main() {}\n"));
    }

    #[test]
    fn in_memory_texts_and_cancellation() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-memory-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class bool {} class num {} class int extends num {} class Null {}",
        )
        .unwrap();
        let a = root.join("a.dart");
        let b = root.join("b.dart");
        // On disk, all fine; in memory, `b.dart` changes the type of `x`.
        std::fs::write(&a, "import 'b.dart';\nvoid f() { if (x) {} }\n").unwrap();
        std::fs::write(&b, "bool x = true;\n").unwrap();
        let engine = Engine::new(&root.join("sdk/lib")).unwrap();
        let non_bool = |r: &AnalysisResult| {
            r.files[&key(&a)]
                .diags
                .iter()
                .filter(|d| d.code == Some(codes::compile_time_error::NON_BOOL_CONDITION))
                .count()
        };
        let disk = engine.analyze(&root, std::slice::from_ref(&a), None);
        assert_eq!(non_bool(&disk), 0);
        let texts = HashMap::from([(key(&b), "int x = 0;\n".to_string())]);
        let memory = engine
            .analyze_with(&root, std::slice::from_ref(&a), None, &texts, &|| false)
            .unwrap();
        assert_eq!(non_bool(&memory), 1, "{:?}", memory.files[&key(&a)].diags);
        assert!(
            engine
                .analyze_with(&root, std::slice::from_ref(&a), None, &texts, &|| true)
                .is_none()
        );
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::path(&sdk, "dartdevc"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A resident analysis whose batch only had bodies edited starts from the
    /// kept program and publishes what a fresh analysis publishes; an API edit,
    /// another batch or a file changed elsewhere loads again.
    #[test]
    fn resident_reuses_the_program_when_only_bodies_change() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-world-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class bool {} class num {} class int extends num {} class Null {}\n\
             class String {}",
        )
        .unwrap();
        let a = root.join("a.dart");
        let b = root.join("b.dart");
        std::fs::write(&b, "bool x = true;\nint y = 0;\n").unwrap();
        std::fs::write(
            &a,
            "import 'b.dart';\nvoid f() {}\nvoid g() {}\nint w = true;\n",
        )
        .unwrap();
        let engine = Engine::new(&root.join("sdk/lib")).unwrap();
        let mut resident = Resident::new();
        let shown = |r: &AnalysisResult| {
            let mut v: Vec<(usize, usize, String)> = r.files[&key(&a)]
                .diags
                .iter()
                .map(|d| (d.span.start, d.span.end, d.message.clone()))
                .collect();
            v.sort();
            v
        };
        let texts_of = |t: &str| HashMap::from([(key(&a), t.to_string())]);
        let steps = [
            // Bodies only: a type error, a syntax error, an unused local, back.
            (
                "import 'b.dart';\nvoid f() { if (y) {} }\nvoid g() {}\nint w = true;\n",
                true,
            ),
            (
                "import 'b.dart';\nvoid f() { if (y) {} }\nvoid g() { int z = 0 }\nint w = true;\n",
                true,
            ),
            (
                "import 'b.dart';\nvoid f() { if (x) {} }\nvoid g() { int z = 0; }\nint w = true;\n",
                true,
            ),
            // The API: a new declaration.
            (
                "import 'b.dart';\nvoid f() { if (x) {} }\nvoid g() {}\nint w = true;\nint h() => 0;\n",
                false,
            ),
            (
                "import 'b.dart';\nvoid f() { if (y) {} }\nvoid g() {}\nint w = true;\nint h() => 0;\n",
                true,
            ),
        ];
        engine.analyze_resident(
            &mut resident,
            &root,
            std::slice::from_ref(&a),
            None,
            &HashMap::new(),
            &|| false,
        );
        for (i, (text, reuse)) in steps.iter().enumerate() {
            let before = resident.reused_worlds;
            let texts = texts_of(text);
            let kept = engine
                .analyze_resident(
                    &mut resident,
                    &root,
                    std::slice::from_ref(&a),
                    None,
                    &texts,
                    &|| false,
                )
                .unwrap();
            let fresh = engine
                .analyze_with(&root, std::slice::from_ref(&a), None, &texts, &|| false)
                .unwrap();
            assert_eq!(shown(&kept), shown(&fresh), "step {i}");
            // The initializer's error, after the edited bodies, moves with them.
            assert!(
                shown(&kept).iter().any(|d| text[d.0..d.1].contains("true")),
                "step {i}: {:?}",
                shown(&kept)
            );
            assert_eq!(
                resident.reused_worlds - before,
                usize::from(*reuse),
                "step {i}"
            );
        }
        // A file outside the batch changed on disk: loads again.
        std::fs::write(&b, "int x = 0;\nint y = 0;\n").unwrap();
        let before = resident.reused_worlds;
        let texts = texts_of(steps[4].0);
        let kept = engine
            .analyze_resident(
                &mut resident,
                &root,
                std::slice::from_ref(&a),
                None,
                &texts,
                &|| false,
            )
            .unwrap();
        let fresh = engine
            .analyze_with(&root, std::slice::from_ref(&a), None, &texts, &|| false)
            .unwrap();
        assert_eq!(shown(&kept), shown(&fresh));
        assert_eq!(resident.reused_worlds, before);
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::path(&sdk, "dartdevc"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// With the package as the resident's context, the batches of the package
    /// share one program: after an API edit of `b.dart`, the analysis of its
    /// dependents starts from the program that `b.dart`'s analysis built, and
    /// publishes what a fresh analysis publishes (a dependent's syntax error,
    /// which that load did not parse, included).
    #[test]
    fn resident_context_shares_the_program_between_batches() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-context-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class bool {} class num {} class int extends num {} class Null {}",
        )
        .unwrap();
        let a = root.join("a.dart");
        let b = root.join("b.dart");
        let c = root.join("c.dart");
        // `a.dart`'s program is most of the package: the whole package pays.
        let padding = format!("// {}\n", "x".repeat(2000));
        std::fs::write(&b, "bool x = true;\n").unwrap();
        std::fs::write(
            &a,
            format!("import 'b.dart';\n{padding}void f() {{ if (x) {{}} }}\n"),
        )
        .unwrap();
        std::fs::write(&c, "import 'b.dart';\nvoid g() { if (x) {} int y = }\n").unwrap();
        let engine = Engine::new(&root.join("sdk/lib")).unwrap();
        let mut resident = Resident::new();
        resident.set_context(vec![a.clone(), b.clone(), c.clone()]);
        let shown = |r: &AnalysisResult, f: &Path| {
            let mut v: Vec<(usize, usize, String)> = r.files[&key(f)]
                .diags
                .iter()
                .map(|d| (d.span.start, d.span.end, d.message.clone()))
                .collect();
            v.sort();
            v
        };
        let none = HashMap::new();
        engine
            .analyze_resident(
                &mut resident,
                &root,
                std::slice::from_ref(&a),
                None,
                &none,
                &|| false,
            )
            .unwrap();
        // The API of `b.dart` changes: its own batch reloads.
        let texts = HashMap::from([(key(&b), "int x = 0;\n".to_string())]);
        let before = resident.reused_worlds;
        engine
            .analyze_resident(
                &mut resident,
                &root,
                std::slice::from_ref(&b),
                None,
                &texts,
                &|| false,
            )
            .unwrap();
        assert_eq!(resident.reused_worlds, before);
        // Its dependents start from that program.
        let dependents = vec![a.clone(), c.clone()];
        let kept = engine
            .analyze_resident(&mut resident, &root, &dependents, None, &texts, &|| false)
            .unwrap();
        assert_eq!(resident.reused_worlds, before + 1);
        let fresh = engine
            .analyze_with(&root, &dependents, None, &texts, &|| false)
            .unwrap();
        for f in [&a, &c] {
            assert_eq!(shown(&kept, f), shown(&fresh, f), "{}", f.display());
            assert!(!shown(&kept, f).is_empty(), "{}", f.display());
        }
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::path(&sdk, "dartdevc"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// API changes in the package reload only what follows the frozen prefix
    /// (the SDK and a library outside the package): the prefix's elements,
    /// types and hierarchy stay, and every batch publishes what an analysis
    /// from scratch does — a changed field type, a changed supertype, a new
    /// class, an override of a prefix member and a generic prefix class.
    #[test]
    fn resident_reloads_only_after_the_frozen_prefix() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-frozen-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("sdk/lib/core")).unwrap();
        std::fs::create_dir_all(base.join("pkg")).unwrap();
        std::fs::create_dir_all(base.join("ext")).unwrap();
        let base = std::fs::canonicalize(&base).unwrap();
        std::fs::write(
            base.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            base.join("sdk/lib/core/core.dart"),
            "class Object {} class bool {} class num {} class int extends num {} class Null {}",
        )
        .unwrap();
        let root = base.join("pkg");
        std::fs::write(
            base.join("ext/e.dart"),
            "class E { int m() => 0; }\nclass G<T> { T? t; }\nclass H extends E {}\n",
        )
        .unwrap();
        let a = root.join("a.dart");
        let b = root.join("b.dart");
        let c = root.join("c.dart");
        std::fs::write(
            &b,
            "import '../ext/e.dart';\nclass B extends E { int x = 0; G<int> g = G(); }\n",
        )
        .unwrap();
        std::fs::write(
            &a,
            "import 'b.dart';\nvoid f(B b) { int y = b.x; int z = b.m(); int? w = b.g.t; }\n",
        )
        .unwrap();
        std::fs::write(&c, "import 'b.dart';\nvoid g(B b) { bool z = b.x; }\n").unwrap();
        // A library of the package that does not depend on `b.dart`: kept by
        // the loads over the prefix, with its diagnostics (an initializer's, an
        // override's and a body's).
        let d = root.join("d.dart");
        std::fs::write(
            &d,
            "import '../ext/e.dart';\nvar v = missing;\nclass D extends E { m() => 1; }\nvoid h() { int k = true; }\n",
        )
        .unwrap();
        let engine = Engine::new(&base.join("sdk/lib")).unwrap();
        let mut resident = Resident::new();
        let shown = |r: &AnalysisResult, f: &Path| {
            let mut v: Vec<(usize, usize, String)> = r.files[&key(f)]
                .diags
                .iter()
                .map(|d| (d.span.start, d.span.end, d.message.clone()))
                .collect();
            v.sort();
            v
        };
        let all = vec![d.clone(), a.clone(), b.clone(), c.clone()];
        let none = HashMap::new();
        engine
            .analyze_resident(&mut resident, &root, &all, None, &none, &|| false)
            .unwrap();
        let versions = [
            "import '../ext/e.dart';\nclass B extends E { bool x = true; G<bool> g = G(); }\n",
            "import '../ext/e.dart';\nclass B extends H { int x = 0; G<int> g = G(); }\nclass C {}\n",
            "import '../ext/e.dart';\nclass B extends E { int x = 0; G<int> g = G(); int m() => 1; }\n",
            "import '../ext/e.dart';\nclass B extends E { int x = 0; G<int> g = G(); m() => 'a'; }\n",
        ];
        for version in versions {
            let texts = HashMap::from([(key(&b), version.to_string())]);
            let before = resident.onto_worlds;
            let own = engine
                .analyze_resident(
                    &mut resident,
                    &root,
                    std::slice::from_ref(&b),
                    None,
                    &texts,
                    &|| false,
                )
                .unwrap();
            assert_eq!(resident.onto_worlds, before + 1, "{version}");
            let rest = vec![a.clone(), c.clone()];
            let dependents = engine
                .analyze_resident(&mut resident, &root, &rest, None, &texts, &|| false)
                .unwrap();
            let fresh = engine
                .analyze_with(&root, &all, None, &texts, &|| false)
                .unwrap();
            assert_eq!(shown(&own, &b), shown(&fresh, &b), "{version}");
            for f in [&a, &c] {
                assert_eq!(shown(&dependents, f), shown(&fresh, f), "{version} {f:?}");
            }
            assert!(resident.kept_libraries > 0, "{version}");
            // A body edit in the kept library, then its own batch.
            let mut texts = texts.clone();
            texts.insert(
                key(&d),
                "import '../ext/e.dart';\nvar v = missing;\nclass D extends E { m() => 1; }\nvoid h() { int k = false; }\n"
                    .to_string(),
            );
            let kept = engine
                .analyze_resident(
                    &mut resident,
                    &root,
                    std::slice::from_ref(&d),
                    None,
                    &texts,
                    &|| false,
                )
                .unwrap();
            let fresh = engine
                .analyze_with(&root, &all, None, &texts, &|| false)
                .unwrap();
            assert!(!shown(&fresh, &d).is_empty());
            assert_eq!(shown(&kept, &d), shown(&fresh, &d), "{version} d");
        }
        // The same API changes over and over: the type table does not grow
        // with the number of loads (each load collects it).
        let mut sizes = Vec::new();
        for round in 0..6 {
            let version = versions[round % 2];
            let texts = HashMap::from([(key(&b), version.to_string())]);
            engine
                .analyze_resident(
                    &mut resident,
                    &root,
                    std::slice::from_ref(&b),
                    None,
                    &texts,
                    &|| false,
                )
                .unwrap();
            sizes.push(resident.world.as_ref().unwrap().table.len());
        }
        assert_eq!(sizes[2], sizes[4], "{sizes:?}");
        assert_eq!(sizes[3], sizes[5], "{sizes:?}");
        let sdk = SdkLayout::load(&base.join("sdk/lib"), "dartdevc").unwrap();
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::path(&sdk, "dartdevc"));
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A package whose whole program is much larger than a batch's (here a
    /// large file nobody imports) is not loaded with every batch: the batches
    /// keep their own programs, and still publish what a fresh analysis does.
    #[test]
    fn resident_skips_the_context_when_the_package_is_much_larger() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-large-context-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class bool {} class num {} class int extends num {} class Null {}",
        )
        .unwrap();
        let a = root.join("a.dart");
        let b = root.join("b.dart");
        let big = root.join("big.dart");
        std::fs::write(&b, "bool x = true;\n").unwrap();
        std::fs::write(&a, "import 'b.dart';\nvoid f() { if (x) {} }\n").unwrap();
        std::fs::write(&big, format!("// {}\nint unused = 0;\n", "x".repeat(5000))).unwrap();
        let mut package = vec![a.clone(), b.clone(), big.clone()];
        for i in 0..3 {
            let other = root.join(format!("other{i}.dart"));
            std::fs::write(&other, format!("int other{i} = {i};\n")).unwrap();
            package.push(other);
        }
        let engine = Engine::new(&root.join("sdk/lib")).unwrap();
        let mut resident = Resident::new();
        resident.set_context(package.clone());
        let none = HashMap::new();
        // The whole package's analysis (at startup) gives its program's size.
        engine
            .analyze_resident(&mut resident, &root, &package, None, &none, &|| false)
            .unwrap();
        for _ in 0..2 {
            let kept = engine
                .analyze_resident(
                    &mut resident,
                    &root,
                    std::slice::from_ref(&a),
                    None,
                    &none,
                    &|| false,
                )
                .unwrap();
            let fresh = engine
                .analyze_with(&root, std::slice::from_ref(&a), None, &none, &|| false)
                .unwrap();
            assert_eq!(kept.files[&key(&a)].diags, fresh.files[&key(&a)].diags);
        }
        // The batch does not bring the whole package in (the kept program may
        // still have it, from the package's analysis: a load over it keeps the
        // libraries that did not change).
        assert!(!resident.context_pays(&root));
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::path(&sdk, "dartdevc"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn conditional_false_uses_absent_analyzer_context_and_private_branch_types() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-conditional-context-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class bool {} class num {} class int extends num {} class Null {}",
        )
        .unwrap();
        std::fs::write(root.join("primary.dart"), "class PublicApi { _Private get value => _Private(); } class _Private { bool get flag => true; }").unwrap();
        std::fs::write(root.join("alternative.dart"), "class PublicApi { _Private get value => _Private(); } class _Private { int get flag => 1; }").unwrap();
        let entry = root.join("consumer.dart");
        let engine = Engine::new(&root.join("sdk/lib")).unwrap();
        for condition in [
            "dart.library.core",
            "dart.library.core == 'false'",
            "dart.library.missing == 'false'",
            "missing.flag == 'false'",
        ] {
            std::fs::write(&entry, format!("import 'primary.dart' if ({condition}) 'alternative.dart'; void consume() {{ if (PublicApi().value.flag) {{}} }}")).unwrap();
            let result = engine.analyze(&root, std::slice::from_ref(&entry), None);
            assert!(
                result.files[&key(&entry)].diags.is_empty(),
                "{condition}: {:?}",
                result.files[&key(&entry)].diags
            );
        }
        // A conditional export must use the same declared-variable context.
        std::fs::write(
            root.join("api.dart"),
            "export 'primary.dart' if (dart.library.core == 'false') 'alternative.dart';",
        )
        .unwrap();
        std::fs::write(
            &entry,
            "import 'api.dart'; void consume() { if (PublicApi().value.flag) {} }",
        )
        .unwrap();
        let result = engine.analyze(&root, std::slice::from_ref(&entry), None);
        assert!(
            result.files[&key(&entry)].diags.is_empty(),
            "{:?}",
            result.files[&key(&entry)].diags
        );
        let _ = std::fs::remove_file(dartforge_elements::SdkCache::path(&engine.sdk, "dartdevc"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inherited_conflict_reaches_the_right_file_in_the_engine() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/engine-inheritance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class int extends Object {}",
        )
        .unwrap();
        let entry = root.join("main.dart");
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/conflicting_static_and_instance/ConflictingStaticAndInstanceClass__inSu_7966ede4.dart"
        ));
        std::fs::write(&entry, source).unwrap();
        let engine = Engine::new(&root.join("sdk/lib")).unwrap();
        let result = engine.analyze(&root, std::slice::from_ref(&entry), None);
        let file = &result.files[&key(&entry)];
        let found: Vec<_> = file
            .diags
            .iter()
            .filter(|d| d.code == Some(codes::compile_time_error::CONFLICTING_STATIC_AND_INSTANCE))
            .collect();
        assert_eq!(found.len(), 1, "{:?}", file.diags);
        assert_eq!(
            &source[found[0].span.start as usize..found[0].span.end as usize],
            "foo"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
