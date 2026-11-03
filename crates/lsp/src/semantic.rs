//! The LSP semantic analyzer. The program, the trees and the type table of a
//! query live, at most, in the bounded session of [`crate::session`] (one
//! entry, discarded at each text change); completion, which analyzes a text
//! with a sentinel, is always transient.

use crate::{Analyzer, DocumentStore, Hover, References, SyntacticAnalyzer};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::{
    generated::GenerationBuilder,
    load::load_lenient_outline,
    model::{Program, UnitId},
    sdk::SdkLayout,
};
use dartforge_intern::Interner;
use url::Url;

/// Analyzer with the resolution and the types of the common inference:
/// definition, hover and references by declaration identity
/// (`crate::project`), completion, rename and actions. Without an SDK, it
/// keeps the syntactic answers.
pub struct SemanticAnalyzer {
    syntactic: SyntacticAnalyzer,
    sdk: Option<SdkLayout>,
    /// The last loaded program, reused while nothing changed.
    session: crate::session::Session,
    /// The wide load of the non-local searches (`references`, `implementation`:
    /// the project, the whole SDK and the packages), retained apart from the
    /// main session so it is not swapped at every `hover`; the bodies of the
    /// libraries outside the project are inferred on demand and stay
    /// (docs/LSP-ESPECIFICACAO.md §11.9 B).
    wide_session: crate::session::Session,
    /// Per project root: the whole wide load fits in the wide session's
    /// budget ([`crate::project::wide_source_estimate`]); otherwise each
    /// reference search loads only what cites its name. Cleared when the
    /// package configuration changes.
    wide_fits: std::collections::HashMap<std::path::PathBuf, bool>,
    /// The binary that runs the reference searches that do not fit
    /// ([`Analyzer::search_in_child_processes`]); `None`: in this process.
    search_exe: Option<std::path::PathBuf>,
    /// The resolved-reference index of the project, once built
    /// ([`crate::reference_index`]).
    reference_index: Option<crate::reference_index::ReferenceIndex>,
    /// The summaries of the known libraries (export namespace) of `ImportLibrary`.
    known: crate::known_libraries::LibraryIndex,
    /// The current `maxCompletionItems`.
    completion_max: usize,
    not_imported_in_completion: bool,
    /// The incremental path (docs/LSP-ESPECIFICACAO.md §16.10 I3 and I4): a
    /// body-only `didChange` swaps the retained unit in place, and completion
    /// infers only the sentinel's body. Turned off by
    /// `DARTFORGE_LSP_INCREMENTAL=0` or [`SemanticAnalyzer::without_incremental`]
    /// (the comparison with the full path).
    incremental: bool,
}

impl SemanticAnalyzer {
    /// Analyzer with the given SDK; without an SDK, only the syntactic answers.
    ///
    /// ```
    /// let a = dartforge_lsp::SemanticAnalyzer::new(None);
    /// assert_eq!(a.session_stats().loaded, 0);
    /// ```
    pub fn new(sdk: Option<SdkLayout>) -> Self {
        Self {
            syntactic: SyntacticAnalyzer::new(),
            sdk,
            session: crate::session::Session::new(),
            wide_session: crate::session::Session::wide(),
            wide_fits: Default::default(),
            search_exe: None,
            reference_index: None,
            known: Default::default(),
            completion_max: crate::completion::MAX_PATTERN,
            not_imported_in_completion: true,
            incremental: std::env::var("DARTFORGE_LSP_INCREMENTAL")
                .map(|v| v.trim() != "0")
                .unwrap_or(true),
        }
    }

    /// Changes the semantic session budget (MiB of retained source; `0` turns
    /// retention off). The default comes from `DARTFORGE_LSP_SESSION_MIB`.
    ///
    /// ```
    /// use dartforge_lsp::SemanticAnalyzer;
    /// let a = SemanticAnalyzer::new(None).with_session_budget(0);
    /// assert_eq!(a.session_stats().retained_source, 0);
    /// ```
    pub fn with_session_budget(mut self, mib: usize) -> Self {
        self.session = crate::session::Session::with_budget(mib);
        self
    }

    /// Without the incremental path: each edit drops the session and completion
    /// loads and infers the whole library.
    ///
    /// ```
    /// let a = dartforge_lsp::SemanticAnalyzer::new(None).without_incremental();
    /// assert_eq!(a.session_stats().body_swaps, 0);
    /// ```
    pub fn without_incremental(mut self) -> Self {
        self.incremental = false;
        self
    }

    /// Counters of the semantic session (reuses, loads, invalidations, retained
    /// source).
    pub fn session_stats(&self) -> crate::session::SessionStats {
        self.session.stats
    }

    /// The library of `uri` (definition, hover, actions), from the session or loaded.
    pub(crate) fn library(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
    ) -> Option<crate::session::Use<'_>> {
        let sdk = self.sdk.as_ref()?;
        let file = crate::project::uri_file(uri)?;
        self.session
            .get(crate::session::Scope::Library(file), documents, || {
                crate::project::load_library(sdk, documents, uri)
            })
    }

    /// The whole project of `uri` (references, rename), from the session or loaded.
    pub(crate) fn project(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
    ) -> Option<crate::session::Use<'_>> {
        let sdk = self.sdk.as_ref()?;
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        self.session
            .get(crate::session::Scope::Project(root), documents, || {
                crate::project::load_project(sdk, documents, uri)
            })
    }

    /// The wide load of `uri` with the bodies of the libraries that cite or
    /// declare `name` inferred (the candidates of `_addResults`, §11.2), from
    /// the wide session or loaded. With `only_citing` and a whole load that
    /// does not fit in the wide session (so it would be loaded again at every
    /// search), the load of that name's search alone (`load_wide_project` with
    /// `citing`): what a reference search needs, not a transitive subtype
    /// search.
    pub(crate) fn wide(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        name: &str,
        only_citing: bool,
    ) -> Option<crate::session::Use<'_>> {
        let SemanticAnalyzer {
            wide_session,
            wide_fits,
            sdk: Some(sdk),
            ..
        } = self
        else {
            return None;
        };
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        let fits = *wide_fits.entry(root.clone()).or_insert_with(|| {
            crate::project::wide_source_estimate(&root) <= wide_session.budget()
        });
        let (scope, citing) = if only_citing && !fits {
            let scope = crate::session::Scope::Search {
                root,
                name: name.to_string(),
            };
            (scope, Some(name))
        } else {
            (crate::session::Scope::Project(root), None)
        };
        let mut usage = wide_session.get(scope, documents, || {
            crate::project::load_wide_project(sdk, documents, uri, citing)
        })?;
        usage.widen_to(name);
        Some(usage)
    }

    /// The reference search for `name` at `offset` of `uri` in a child process,
    /// when the project's whole wide load does not fit in the wide session (the
    /// search would load and drop it in this process, which then stays at its
    /// peak: the system allocator does not give the pages back, docs/LSP.md
    /// "Memória"). `None` when it does fit, without a child binary, or when the
    /// child fails (the caller searches here).
    fn search_in_child(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        name: &str,
    ) -> Option<Option<References>> {
        let exe = self.search_exe.clone()?;
        let sdk = self.sdk.as_ref()?;
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        let budget = self.wide_session.budget();
        let fits = *self
            .wide_fits
            .entry(root.clone())
            .or_insert_with(|| crate::project::wide_source_estimate(&root) <= budget);
        if fits {
            return None;
        }
        let request = SearchRequest {
            sdk_lib: sdk.root.clone(),
            uri: uri.to_string(),
            offset,
            name: name.to_string(),
            documents: documents
                .uris()
                .filter_map(|u| {
                    Some((
                        u.to_string(),
                        documents.version(u).unwrap_or(0),
                        documents.get(u)?.to_string(),
                    ))
                })
                .collect(),
        };
        let found = run_search_child(&exe, &request);
        if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
            eprintln!(
                "[dartforge-lsp] reference search for {name} in a child: {}",
                if found.is_some() {
                    "answered"
                } else {
                    "failed"
                }
            );
        }
        found.map(|r| {
            r.map(|f| References {
                declaration: f.declaration,
                uses: f.uses,
                ..Default::default()
            })
        })
    }

    /// Discovers the SDK through the same paths the compiler uses.
    pub fn discover() -> Self {
        Self::discover_reporting().0
    }

    /// [`SemanticAnalyzer::discover`], with the report of what was chosen and
    /// what was refused, for the client's log ([`crate::SdkReport`]).
    ///
    /// A candidate that exists but does not load is refused (and listed in
    /// the report) and the next one is tried.
    pub fn discover_reporting() -> (Self, crate::SdkReport) {
        let mut sdk = None;
        let search = dartforge_elements::sdk_discovery::search_with(&mut |lib| {
            sdk = Some(load_sdk(lib)?);
            Ok(())
        });
        (Self::new(sdk), crate::SdkReport::from_search(search, None))
    }

    /// In-memory generation with the current texts of the open `.dart`
    /// documents: during the load, they win over the disk.
    pub(crate) fn open(documents: &DocumentStore) -> GenerationBuilder {
        let mut generator = GenerationBuilder::new();
        for open in documents.uris() {
            let Some(source) = documents.get(open) else {
                continue;
            };
            // The canonical path (the disk's case on a case-insensitive file system),
            // the same as `uri_file`'s.
            let Some(file) = crate::project::uri_file(open) else {
                continue;
            };
            if file.extension().is_some_and(|e| e == "dart") {
                generator.put(file, source.to_owned(), "lsp", vec![]);
            }
        }
        generator
    }

    /// A [`DocumentStore`] with only `uri`, for the trait calls that receive the
    /// text without the collection of open documents.
    fn only_this(uri: &str, text: &str) -> DocumentStore {
        let mut docs = DocumentStore::new();
        docs.open(uri.to_string(), 0, text.to_string());
        docs
    }

    /// Definition through semantic identity (`crate::project`): the directive
    /// literal stays syntactic (the pointed-to file); names come from the common
    /// inference's resolution. Without an SDK, or if the library does not load,
    /// the conservative syntactic answers apply.
    fn define(
        &mut self,
        uri: &str,
        text: &str,
        offset: usize,
        documents: &DocumentStore,
    ) -> Option<(String, Option<Span>)> {
        if let Some((target, None)) = self.syntactic.definition(uri, text, offset) {
            return Some((target, None));
        }
        if let Some(project) = self.library(documents, uri)
            && let Some(unit) = project.uri_unit(uri)
            && let Ok(Some(_)) = project.identify(unit, offset)
        {
            let (u, span) = project.definition(unit, offset)?;
            return Some((project.unit_uri(u)?, Some(span)));
        }
        self.syntactic
            .definition_in_workspace(uri, text, offset, documents)
    }

    /// Hover through semantic identity; without it, the conservative syntactic one.
    fn pass_hover(
        &mut self,
        uri: &str,
        text: &str,
        offset: usize,
        documents: &DocumentStore,
    ) -> Option<Hover> {
        if let Some(project) = self.library(documents, uri)
            && let Some(unit) = project.uri_unit(uri)
            && let Ok(Some(d)) = project.identify(unit, offset)
            && let Some(h) = project.hover(unit, &d)
        {
            return Some(h);
        }
        if let Some(project) = self.library(documents, uri)
            && let Some(unit) = project.uri_unit(uri)
            && let Some(h) = project.invalid_hover(unit, offset)
        {
            return Some(h);
        }
        self.syntactic
            .hover_in_workspace(uri, text, offset, documents)
    }

    /// References through declaration identity, in every library of the project
    /// (open or only on disk). Declarations from dependencies (SDK, packages)
    /// come in as declarations, but uses are only searched for in the project.
    fn semantic_references(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<References> {
        // Non-local search: the wide load, with the SDK and the packages that cite
        // the name (docs/LSP-ESPECIFICACAO.md §11.9 B); a local one stays in the
        // file's library. What is at the cursor resolves the same in the
        // library's load (the file and what it imports) as in the whole
        // project's, so the library names the search.
        let name = {
            let project = self.library(documents, uri)?;
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)
        };
        let Some(name) = name else {
            // A local, a type parameter or a prefix: only this file has them.
            let library = self.library(documents, uri)?;
            return references_in(&library, uri, offset);
        };
        // A private name is only visible in its library, whose files are the
        // only candidates (`_addResults`): the library's load has them all.
        if name.starts_with('_') {
            let library = self.library(documents, uri)?;
            return references_in(&library, uri, offset);
        }
        if let Ok(found) = self.indexed_references(documents, uri, offset) {
            return found;
        }
        if let Some(found) = self.search_in_child(documents, uri, offset, &name) {
            return found;
        }
        if let Some(wide) = self.wide(documents, uri, &name, true) {
            return references_in(&wide, uri, offset);
        }
        let project = self.project(documents, uri)?;
        references_in(&project, uri, offset)
    }
}

impl SemanticAnalyzer {
    /// The references from the index ([`crate::reference_index`]): what is at
    /// `offset` is identified in the document's library and the index answers.
    /// `Err` when there is no index or it does not answer (the caller searches
    /// as before).
    fn indexed_references(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Result<Option<References>, crate::reference_index::Miss> {
        use crate::reference_index::Miss;
        let mut index = self
            .reference_index
            .take()
            .ok_or(Miss::Unsupported("no index"))?;
        let answer = (|| {
            let library = self
                .library(documents, uri)
                .ok_or(Miss::Unsupported("no library"))?;
            let unit = library.uri_unit(uri).ok_or(Miss::Unsupported("no unit"))?;
            let Ok(Some(d)) = library.identify(unit, offset) else {
                return Ok(None);
            };
            let fresh = |r: &crate::reference_index::FileRecord| record_is_current(r, documents);
            let (declaration, uses) = index.references(&library, &d, &fresh)?;
            let declaration_range = declaration.as_ref().and_then(|d| d.2);
            let use_ranges: Vec<(u32, u32, u32, u32)> = uses.iter().filter_map(|u| u.2).collect();
            Ok(Some(References {
                declaration: declaration.map(|(u, s, _)| (u, s)),
                uses: uses.into_iter().map(|(u, s, _)| (u, s)).collect(),
                declaration_range,
                use_ranges,
            }))
        })();
        self.reference_index = Some(index);
        if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some()
            && let Err(miss) = &answer
        {
            eprintln!("[dartforge-lsp] the reference index does not answer: {miss:?}");
        }
        answer
    }

    /// Builds the reference index of the project of `uri` in this process
    /// (the comparison of [`SemanticAnalyzer::check_reference_index`]).
    fn build_reference_index_here(&mut self, documents: &DocumentStore, uri: &str) -> bool {
        let Some(sdk) = self.sdk.as_ref() else {
            return false;
        };
        self.reference_index = crate::reference_index::build(sdk, documents, uri);
        self.reference_index.is_some()
    }

    /// Compares, at every identifier of the open documents `uris`, the
    /// references the index gives with those of the search without it (one
    /// position per target and file). The report has the counts and the first
    /// differences.
    #[doc(hidden)]
    pub fn check_reference_index(&mut self, documents: &DocumentStore, uris: &[String]) -> String {
        use std::fmt::Write as _;
        type Answer = Option<(Option<(String, Span)>, Vec<(String, Span)>)>;
        let mut report = String::new();
        let Some(first) = uris.first() else {
            return report;
        };
        let started = std::time::Instant::now();
        if !self.build_reference_index_here(documents, first) {
            return "the index could not be built".to_string();
        }
        let _ = writeln!(
            report,
            "index built in {} ms",
            started.elapsed().as_millis()
        );
        let mut compared = 0usize;
        let mut equal = 0usize;
        let mut misses: std::collections::BTreeMap<String, usize> = Default::default();
        let mut differences = Vec::new();
        let (mut old_time, mut new_time) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
        let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
        for uri in uris {
            let Some(text) = documents.get(uri).map(str::to_string) else {
                continue;
            };
            let mut seen: std::collections::HashSet<String> = Default::default();
            let b = text.as_bytes();
            for offset in 0..b.len() {
                if !ident(b[offset])
                    || b[offset].is_ascii_digit()
                    || (offset > 0 && ident(b[offset - 1]))
                {
                    continue;
                }
                let target = {
                    let Some(library) = self.library(documents, uri) else {
                        continue;
                    };
                    let Some(unit) = library.uri_unit(uri) else {
                        continue;
                    };
                    let Ok(Some(d)) = library.identify(unit, offset) else {
                        continue;
                    };
                    if library.search_name(&d.target).is_none() {
                        continue;
                    }
                    format!("{:?}", d.target)
                };
                if !seen.insert(target.clone()) {
                    continue;
                }
                let t = std::time::Instant::now();
                let private = {
                    let library = self.library(documents, uri);
                    library.is_some_and(|l| {
                        l.uri_unit(uri)
                            .and_then(|u| l.identify(u, offset).ok().flatten())
                            .and_then(|d| l.search_name(&d.target))
                            .is_some_and(|n| n.starts_with('_'))
                    })
                };
                let new = if private {
                    // The private shortcut of `semantic_references`.
                    let library = self.library(documents, uri);
                    Ok(library.and_then(|l| references_in(&l, uri, offset)))
                } else {
                    self.indexed_references(documents, uri, offset)
                };
                new_time += t.elapsed();
                let new = match new {
                    Ok(n) => n,
                    Err(miss) => {
                        let reason = match miss {
                            crate::reference_index::Miss::Unsupported(r) => r.to_string(),
                            crate::reference_index::Miss::Key(k) => {
                                format!(
                                    "a key: {}",
                                    k.split(['#', '.']).take(3).collect::<Vec<_>>().join("|")
                                )
                            }
                            crate::reference_index::Miss::Stale(_) => "stale".to_string(),
                        };
                        *misses.entry(reason).or_default() += 1;
                        continue;
                    }
                };
                // The search without the index nor the private shortcut.
                let index = self.reference_index.take();
                let t = std::time::Instant::now();
                let old = self.unindexed_references(documents, uri, offset);
                old_time += t.elapsed();
                self.reference_index = index;
                compared += 1;
                let strip =
                    |r: Option<References>| -> Answer { r.map(|r| (r.declaration, r.uses)) };
                let (new, old) = (strip(new), strip(old));
                if new == old {
                    equal += 1;
                } else if differences.len() < 30 {
                    let (line, column) = documents
                        .lines(uri)
                        .map(|l| l.offset_position(&text, offset))
                        .unwrap_or_default();
                    let count = |r: &Answer| r.as_ref().map_or(0, |r| r.1.len());
                    let only = |a: &Answer, b: &Answer| {
                        let empty = Vec::new();
                        let bu = b.as_ref().map_or(&empty, |r| &r.1);
                        a.as_ref()
                            .map_or(&empty, |r| &r.1)
                            .iter()
                            .filter(|x| !bu.contains(x))
                            .take(3)
                            .map(|(u, s)| {
                                format!("{}@{}", u.rsplit('/').next().unwrap_or(u), s.start)
                            })
                            .collect::<Vec<_>>()
                    };
                    differences.push(format!(
                        "{uri}:{line}:{column} {target}: index {} uses, search {}; only index {:?}; only search {:?}; declarations {:?} / {:?}",
                        count(&new),
                        count(&old),
                        only(&new, &old),
                        only(&old, &new),
                        new.as_ref().and_then(|r| r.0.as_ref().map(|d| d.1.start)),
                        old.as_ref().and_then(|r| r.0.as_ref().map(|d| d.1.start)),
                    ));
                }
            }
        }
        let _ = writeln!(
            report,
            "compared {compared}, equal {equal}, different {}; index {} ms, search {} ms",
            compared - equal,
            new_time.as_millis(),
            old_time.as_millis()
        );
        for (reason, n) in misses {
            let _ = writeln!(report, "not answered ({reason}): {n}");
        }
        for d in differences {
            let _ = writeln!(report, "{d}");
        }
        report
    }

    /// The non-local search without the index nor the private shortcut (the
    /// reference of [`SemanticAnalyzer::check_reference_index`]).
    fn unindexed_references(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<References> {
        let name = {
            let project = self.library(documents, uri)?;
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)?
        };
        if let Some(found) = self.search_in_child(documents, uri, offset, &name) {
            return found;
        }
        if let Some(wide) = self.wide(documents, uri, &name, true) {
            return references_in(&wide, uri, offset);
        }
        let project = self.project(documents, uri)?;
        references_in(&project, uri, offset)
    }
}

/// The record of a file is of the text the server sees: the open document's,
/// or the disk's for a closed file.
fn record_is_current(r: &crate::reference_index::FileRecord, documents: &DocumentStore) -> bool {
    let open = url::Url::from_file_path(&r.path)
        .ok()
        .and_then(|u| documents.get(u.as_str()).map(str::to_string));
    let text = match open {
        Some(t) => t,
        None => match std::fs::read_to_string(&r.path) {
            Ok(t) => t,
            Err(_) => return false,
        },
    };
    text.len() as u32 == r.length && crate::reference_index::text_stamp(&text) == r.stamp
}

/// The hidden argument that makes the `dartforge-lsp` binary a reference
/// search process ([`reference_search_child`]).
pub const REFERENCE_SEARCH_ARGUMENT: &str = "--reference-search";

/// A reference search as the server sends it to the child: the SDK, the
/// position, the searched name and every open document.
#[derive(serde::Serialize, serde::Deserialize)]
struct SearchRequest {
    sdk_lib: std::path::PathBuf,
    uri: String,
    offset: usize,
    name: String,
    documents: Vec<(String, i32, String)>,
}

/// What the child answers: the [`References`], or nothing found.
#[derive(serde::Serialize, serde::Deserialize)]
struct SearchFound {
    declaration: Option<(String, Span)>,
    uses: Vec<(String, Span)>,
}

/// Runs `request` in a child process of `exe`. `None` when the child could not
/// be used; otherwise its answer.
fn run_search_child(exe: &std::path::Path, request: &SearchRequest) -> Option<Option<SearchFound>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(exe)
        .arg(REFERENCE_SEARCH_ARGUMENT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    let input = serde_json::to_vec(request).ok()?;
    let mut stdin = child.stdin.take()?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().ok()?;
    let _ = writer.join();
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

/// The reference search child process ([`REFERENCE_SEARCH_ARGUMENT`]): reads a
/// search from `stdin`, loads what it needs (`load_wide_project` with the
/// name), searches and writes the answer to `stdout`.
pub fn reference_search_child() -> i32 {
    use std::io::Read;
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        return 2;
    }
    let Ok(request) = serde_json::from_slice::<SearchRequest>(&input) else {
        return 2;
    };
    let Ok(sdk) = load_sdk(&request.sdk_lib) else {
        return 2;
    };
    let mut documents = DocumentStore::new();
    for (uri, version, text) in request.documents {
        documents.open(uri, version, text);
    }
    let found =
        crate::project::load_wide_project(&sdk, &documents, &request.uri, Some(&request.name))
            .and_then(|mut project| {
                project.widen_to(&request.name);
                references_in(&project, &request.uri, request.offset)
            })
            .map(|r| SearchFound {
                declaration: r.declaration,
                uses: r.uses,
            });
    match serde_json::to_writer(std::io::stdout().lock(), &found) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Loads the SDK whose `lib/` is `lib` as the analyzer sees it (M1: without
/// patches, with the dart2js internal libraries) and checks that `dart:core`
/// can be read, so that an SDK that would only fail on the first analysis is
/// refused at once.
///
/// # Errors
/// Why the SDK is not usable: an unreadable or invalid `libraries.json`,
/// no `dart:core`, or a `core.dart` that cannot be read.
fn load_sdk(lib: &std::path::Path) -> Result<SdkLayout, String> {
    let sdk = SdkLayout::load_as_analyzer(lib)?;
    let core = sdk
        .library("core")
        .ok_or_else(|| "it declares no dart:core library".to_string())?;
    std::fs::read_to_string(&core.path)
        .map_err(|e| format!("could not read {}: {e}", core.path.display()))?;
    Ok(sdk)
}

/// The references of what is at `offset` of `uri`, in the given project.
fn references_in(
    project: &crate::project::Project,
    uri: &str,
    offset: usize,
) -> Option<References> {
    {
        let unit = project.uri_unit(uri)?;
        let d = project.identify(unit, offset).ok()??;
        if let crate::project::Target::Prefix { library, name } = d.target {
            let (declaration, uses) = project.prefix_references(unit, offset, library, name)?;
            let mut uses: Vec<(String, Span)> = uses
                .into_iter()
                .filter_map(|(u, s)| Some((project.unit_uri(u)?, s)))
                .collect();
            uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
            let declaration = declaration.and_then(|(u, s)| Some((project.unit_uri(u)?, s)));
            return Some(References {
                declaration,
                uses,
                ..Default::default()
            });
        }
        let declaration = project.declaration_to_references(&d);
        let declarations = project.declarations(&d.target);
        // Non-local search: only the candidate files of `_addResults`
        // (docs/LSP-ESPECIFICACAO.md §11.2), including Dart's gaps (factory
        // redirection, operators outside the library).
        let candidates = project.search_name(&d.target).map(|name| {
            let mut units: Vec<UnitId> = declarations.iter().map(|(u, _, _)| *u).collect();
            if let Some((u, _)) = project.declaration(&d) {
                units.push(u);
            }
            units.dedup();
            project.candidate_files(&name, &units)
        });
        let mut uses: Vec<(String, Span)> = Vec::new();
        // Only the candidates accept uses: the scan stays within them.
        for (u, from, until) in project
            .occurrences_in(&d.target, false, candidates.as_ref())
            .ok()?
        {
            if declarations.contains(&(u, from, until)) {
                continue;
            }
            if candidates.as_ref().is_some_and(|c| !c.contains(&u)) {
                continue;
            }
            uses.push((
                project.unit_uri(u)?,
                Span {
                    start: from,
                    end: until,
                },
            ));
        }
        uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        let declaration = declaration.and_then(|(u, s)| Some((project.unit_uri(u)?, s)));
        Some(References {
            declaration,
            uses,
            ..Default::default()
        })
    }
}

/// Loads the program of `uri` with `text` and the other open documents with
/// their current texts (the other files come from disk). A part comes in
/// through its owning library.
pub(crate) fn load(
    sdk: &SdkLayout,
    uri: &str,
    text: &str,
    documents: Option<&DocumentStore>,
) -> Option<(Program, Interner, UnitId)> {
    let path = crate::project::uri_file(uri)?;
    if path.extension().is_none_or(|e| e != "dart") {
        return None;
    }
    let mut generator = documents.map_or_else(GenerationBuilder::new, SemanticAnalyzer::open);
    // A direct trait call also uses the received text, even without a
    // DocumentStore. The request's file wins over the collection.
    generator.put(path.clone(), text.to_owned(), "lsp", vec![]);
    // A part is not a library: the load goes through the owner, which includes
    // it. If the owner does not declare it (missing `part`), the part comes in
    // alone.
    let owning = owning_library(&path, text, documents);
    let generation = generator.finish(1).ok()?;
    let key = dartforge_elements::generated::key(&path);
    // Only this library's bodies are inferred: the libraries outside the
    // project (the SDK and the packages, which no query here edits or shows
    // the bodies of) come in without them. The project's keep theirs, for the
    // queries that look into another project file (`colors`, the assists).
    let root = crate::project::project_root(&path);
    let outline = |p: &std::path::Path, _: &str| !crate::project::inside(p, &root);
    for entry in owning.iter().chain([&path]) {
        let mut names = Interner::new();
        let (program, _) = load_lenient_outline(
            &[entry],
            sdk,
            &mut names,
            Some(generation.clone()),
            &outline,
        );
        let unit = program.units.iter().position(|u| {
            u.path
                .as_deref()
                .map(dartforge_elements::generated::key)
                .as_ref()
                == Some(&key)
        });
        if let Some(unit) = unit {
            return Some((program, names, UnitId(unit as u32)));
        }
    }
    None
}

/// The owning library of `path` when the text is a part (`part of`): through
/// the written URI (relative or `package:`), or, in the old `part of name;`
/// form, through the project file that declares `part` for it.
pub(crate) fn owning_library(
    path: &std::path::Path,
    text: &str,
    documents: Option<&DocumentStore>,
) -> Option<std::path::PathBuf> {
    use dartforge_elements::generated::key;
    use dartforge_frontend::ast::DirectiveKind;
    if !text.contains("part") {
        return None;
    }
    let mut names = Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    let uri = analyzed
        .unit
        .directives
        .iter()
        .find_map(|d| match &d.kind {
            DirectiveKind::PartOf { uri, .. } => Some(
                uri.as_ref()
                    .and_then(dartforge_elements::load::string_lit_value),
            ),
            _ => None,
        })?;
    if let Some(uri) = uri {
        if uri.starts_with("package:") {
            let config = dartforge_elements::config::PackageConfig::discover(path)?;
            return dartforge_elements::config::PackageConfig::load(&config)
                .ok()?
                .resolve_package_uri(&uri)
                .ok();
        }
        return Some(key(&path.parent()?.join(uri)));
    }
    let target = key(path);
    let root = crate::project::project_root(path);
    crate::project::project_files(&root)
        .into_iter()
        .find(|candidate| {
            let open = documents.and_then(|d| {
                Url::from_file_path(candidate)
                    .ok()
                    .and_then(|u| d.get(u.as_str()).map(str::to_string))
            });
            let Some(source) = open.or_else(|| std::fs::read_to_string(candidate).ok()) else {
                return false;
            };
            if !source.contains("part") {
                return false;
            }
            let mut names = Interner::new();
            let analyzed = dartforge_frontend::parser::parse(&source, &mut names);
            analyzed.unit.directives.iter().any(|d| match &d.kind {
                DirectiveKind::Part { uri } => dartforge_elements::load::string_lit_value(uri)
                    .and_then(|u| candidate.parent().map(|dir| key(&dir.join(u))))
                    .is_some_and(|p| p == target),
                _ => false,
            })
        })
}

impl Analyzer for SemanticAnalyzer {
    fn diagnose(&mut self, uri: &str, text: &str) -> Vec<Diagnostic> {
        self.syntactic.diagnose(uri, text)
    }

    fn symbols(&mut self, uri: &str, text: &str) -> Vec<serde_json::Value> {
        self.syntactic.symbols(uri, text)
    }

    fn definition(
        &mut self,
        uri: &str,
        text: &str,
        offset: usize,
    ) -> Option<(String, Option<Span>)> {
        let only_this = Self::only_this(uri, text);
        self.define(uri, text, offset, &only_this)
    }

    fn definition_in_workspace(
        &mut self,
        uri: &str,
        text: &str,
        offset: usize,
        documents: &DocumentStore,
    ) -> Option<(String, Option<Span>)> {
        self.define(uri, text, offset, documents)
    }

    fn references(&mut self, uri: &str, text: &str, offset: usize) -> Option<Vec<Span>> {
        self.syntactic.references(uri, text, offset)
    }

    fn references_in(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<References> {
        if self.sdk.is_some() && documents.get(uri).is_some() {
            return self.semantic_references(documents, uri, offset);
        }
        self.syntactic.references_in(documents, uri, offset)
    }

    fn hover(&mut self, uri: &str, text: &str, offset: usize) -> Option<Hover> {
        let only_this = Self::only_this(uri, text);
        self.pass_hover(uri, text, offset, &only_this)
    }

    fn hover_in_workspace(
        &mut self,
        uri: &str,
        text: &str,
        offset: usize,
        documents: &DocumentStore,
    ) -> Option<Hover> {
        self.pass_hover(uri, text, offset, documents)
    }

    fn closed_document(&mut self, uri: &str) {
        self.syntactic.closed_document(uri);
        self.session.invalidate();
        self.wide_session.invalidate();
    }

    fn changed_document(&mut self, _uri: &str) {
        self.session.invalidate();
        self.wide_session.invalidate();
    }

    fn search_in_child_processes(&mut self, exe: std::path::PathBuf) {
        self.search_exe = Some(exe);
    }

    fn project_configuration_changed(&mut self) {
        self.syntactic.project_configuration_changed();
        self.session.invalidate();
        self.wide_session.invalidate();
        self.wide_fits.clear();
        self.known = Default::default();
    }

    fn open_document(&mut self, documents: &DocumentStore, uri: &str) {
        if self.incremental {
            self.session.open(documents, uri);
            self.wide_session.open(documents, uri);
        } else {
            self.changed_document(uri);
        }
    }

    fn document_closed_at(&mut self, documents: &DocumentStore, uri: &str) {
        self.syntactic.closed_document(uri);
        if self.incremental {
            self.session.close(documents, uri);
            self.wide_session.close(documents, uri);
        } else {
            self.session.invalidate();
            self.wide_session.invalidate();
        }
    }

    fn edited_document(&mut self, documents: &DocumentStore, uri: &str) {
        if self.incremental {
            self.session.update(documents, uri);
            self.wide_session.update(documents, uri);
        } else {
            self.session.invalidate();
            self.wide_session.invalidate();
        }
    }

    fn sdk_for_diagnostics(&self) -> Option<std::path::PathBuf> {
        self.sdk.as_ref().map(|s| s.root.clone())
    }

    fn use_sdk(&mut self, lib: &std::path::Path) -> Result<(), String> {
        let sdk = load_sdk(lib)?;
        self.sdk = Some(sdk);
        self.session.invalidate();
        self.wide_session.invalidate();
        self.known = Default::default();
        Ok(())
    }

    fn prepare_rename(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Result<Option<(Span, String)>, String> {
        // `getElementToRename` and `checkInitialConditions` look only at the
        // element under the cursor: the library's load is enough (the rename
        // itself loads the whole project).
        let Some(project) = self.library(documents, uri) else {
            return Ok(None);
        };
        crate::rename::prepare(&project, uri, offset)
    }

    fn rename(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        new: &str,
    ) -> Result<crate::Rename, String> {
        let project = self.project(documents, uri).ok_or_else(|| {
            "The file's project could not be loaded (missing SDK or a URI that is not a file)."
                .to_string()
        })?;
        crate::rename::rename(&project, uri, offset, new)
    }

    fn actions(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        start: usize,
        end: usize,
        published_: &[Diagnostic],
    ) -> Vec<crate::CodeAction> {
        let Some(text) = documents.get(uri) else {
            return Vec::new();
        };
        // The immediate ones of the current version and the typed ones already
        // published for it.
        let mut diagnostics = self.diagnose(uri, text);
        for d in published_ {
            if !diagnostics
                .iter()
                .any(|x| x.code == d.code && x.span == d.span)
            {
                diagnostics.push(d.clone());
            }
        }
        // The fixes apply to the diagnostics of the requested lines (the Dart
        // server filters by line); the assists, to the cursor.
        let (line_start_, line_end) = {
            let a = text[..start.min(text.len())]
                .rfind('\n')
                .map_or(0, |i| i + 1);
            let b = text[end.min(text.len())..]
                .find('\n')
                .map_or(text.len(), |i| end + i);
            (a, b)
        };
        let mut output = crate::actions::fix_syntax(uri, text, &diagnostics, line_start_, line_end);
        // "Fix all in file" of the syntactic ones.
        let all = crate::actions::fix_in_whole_file(&output, &diagnostics, text, |o, a, b| {
            crate::actions::fix_syntax(uri, text, std::slice::from_ref(o), a, b)
        });
        output.extend(all);
        output.push(crate::actions::organize_imports(uri, text));
        // The ignore ones come last, like Dart's priority.
        let ignore =
            crate::ignore_diagnostic::ignore(uri, text, &diagnostics, line_start_, line_end);
        if self.sdk.is_none() {
            output.extend(ignore);
            return output;
        }
        let Some(file) = crate::project::uri_file(uri) else {
            output.extend(ignore);
            return output;
        };
        let SemanticAnalyzer {
            session,
            sdk: Some(sdk),
            known,
            ..
        } = self
        else {
            output.extend(ignore);
            return output;
        };
        if let Some(mut project) =
            session.get(crate::session::Scope::Library(file), documents, || {
                crate::project::load_library(sdk, documents, uri)
            })
        {
            let published =
                crate::actions::fix_published(&project, uri, &diagnostics, line_start_, line_end);
            let all =
                crate::actions::fix_in_whole_file(&published, &diagnostics, text, |o, a, b| {
                    crate::actions::fix_published(&project, uri, std::slice::from_ref(o), a, b)
                });
            output.extend(published);
            output.extend(all);
            // Import before create: the priority of Dart's fixes.
            output.extend(crate::actions::import(
                &project,
                Some(&*sdk),
                known,
                documents,
                uri,
                line_start_,
                line_end,
            ));
            let fixed = crate::fixes::fix(&mut project, uri, &diagnostics, line_start_, line_end);
            let all = crate::actions::fix_in_whole_file(&fixed, &diagnostics, text, |o, a, b| {
                crate::fixes::fix(&mut project, uri, std::slice::from_ref(o), a, b)
            });
            output.extend(fixed);
            output.extend(all);
            output.extend(crate::fixes::required_arguments(
                &project,
                uri,
                line_start_,
                line_end,
            ));
            if let Some(unit) = project.uri_unit(uri) {
                let created = project.create_undefined(uri, unit, line_start_, line_end);
                output.extend(created);
            }
            if let Some(unit) = project.uri_unit(uri) {
                {
                    let cx = crate::refactorings::Context::new(&project, unit);
                    output.extend(cx.import_add_show(uri, start, end));
                    output.extend(cx.convert_to_package_import(uri, start, end));
                    output.extend(cx.convert_to_relative_import(uri, start, end));
                    output.extend(cx.convert_part_of_to_uri(uri, start, end));
                    output.extend(cx.add_return_type(uri, start, end));
                    output.extend(cx.add_type_annotation(uri, start, end));
                    output.extend(cx.convert_class_to_mixin(uri, start, end));
                    output.extend(cx.convert_class_to_enum(uri, start, end));
                    output.extend(cx.add_diagnostic_reference(uri, start, end));
                    output.extend(cx.flutter_assists(uri, start, end));
                    output.extend(cx.flutter_to_stateful(uri, start, end));
                    output.extend(cx.flutter_to_stateless(uri, start, end));
                    output.extend(cx.convert_to_async_body(uri, start, end));
                    // `ConvertIntoBlockBody.missingBody` as an assist: the selection node
                    // (`NodeLocator`).
                    if let Some((span, text)) =
                        cx.convert_to_block_body(dartforge_diagnostics::Span { start, end })
                    {
                        output.push(crate::fixes::block_body(crate::actions::CodeAction {
                            title: "Convert to block body".into(),
                            kind: "refactor.convert.bodyToBlock".into(),
                            edits: vec![crate::Edit {
                                uri: uri.to_string(),
                                span,
                                text,
                            }],
                            diagnostic: None,
                            create_file: None,
                            linked: Default::default(),
                        }));
                    }
                    output.extend(cx.convert_to_final_field(uri, start, end));
                    output.extend(cx.convert_to_for_index(uri, start, end));
                    output.extend(cx.convert_to_getter(uri, start, end));
                    output.extend(cx.convert_to_normal_parameter(uri, start, end));
                    output.extend(cx.encapsulate_field(uri, start, end));
                    output.extend(cx.convert_to_field_parameter(uri, start, end));
                    output.extend(cx.shadow_field(uri, start, end));
                    output.extend(cx.convert_to_super_parameters(uri, start, end));
                    output.extend(cx.remove_type_annotation(uri, start, end));
                    output.extend(cx.conditional_to_if_else(uri, start, end));
                    output.extend(cx.convert_text_quotes(uri, start, end));
                    output.extend(cx.convert_to_is_not_empty(uri, start, end));
                    output.extend(cx.convert_to_int_literal(uri, start, end));
                    output.extend(cx.swap_to_var(uri, start, end));
                    output.extend(cx.convert_to_function_syntax(uri, start, end));
                    output.extend(cx.wrap_commands(uri, start, end));
                    output.extend(cx.destructure_local(uri, start, end));
                    output.extend(cx.convert_to_if_case(uri, start, end));
                    output.extend(cx.switch_to_if_case_chain(uri, start, end));
                    output.extend(cx.if_to_switch(uri, start, end));
                    output.extend(cx.switch_expression_to_statement(uri, start, end));
                    output.extend(cx.switch_to_expression(uri, start, end));
                    output.extend(cx.split_and_condition(uri, start, end));
                    output.extend(cx.convert_to_null_aware(uri, start, end));
                    output.extend(cx.convert_to_multiline(uri, start, end));
                    output.extend(cx.convert_add_all_to_spread(uri, start, end));
                    output.extend(cx.inline_invocation_add(uri, start, end));
                    output.extend(cx.conditional_to_if_element(uri, start, end));
                    output.extend(cx.block_documentation(uri, start, end));
                    output.extend(cx.line_documentation(uri, start, end));
                    output.extend(cx.convert_to_map_literal(uri, start, end));
                    output.extend(cx.convert_to_set_literal(uri, start, end));
                    output.extend(cx.map_from_iterable_to_for(uri, start, end));
                }
                output.extend(project.rewrite_assists(uri, unit, start, end));
                output.extend(project.syntactic_assists(uri, unit, start));
                output.extend(project.condition_assists(uri, unit, start));
                output.extend(project.join_assists(uri, unit, start));
            }
        }
        output.extend(ignore);
        output
    }

    fn refactorings(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        length: usize,
        create_files: bool,
    ) -> Vec<crate::Refactoring> {
        let Some(project) = self.library(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project.refactorings(unit, offset, length, create_files)
    }

    fn run_refactoring(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        request: &crate::RefactoringRequest,
    ) -> crate::RefactoringResult {
        let Some(project) = self.project(documents, uri) else {
            return crate::RefactoringResult::NotAnalyzed;
        };
        crate::refactorings_exec::run(&project, uri, request)
    }

    fn move_to_file(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        length: usize,
        target: &str,
    ) -> crate::RefactoringResult {
        let Some(project) = self.project(documents, uri) else {
            return crate::RefactoringResult::NotAnalyzed;
        };
        crate::refactorings_exec::move_(&project, uri, offset, length, target)
    }

    fn folds(&mut self, uri: &str, text: &str, lines_only: bool) -> Vec<crate::Fold> {
        self.syntactic.folds(uri, text, lines_only)
    }

    fn selections(&mut self, uri: &str, text: &str, offset: usize) -> Vec<Span> {
        self.syntactic.selections(uri, text, offset)
    }

    fn signature(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        auto: bool,
    ) -> Option<crate::Signature> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.signature(unit, offset, auto)
    }

    fn document_highlights(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<Vec<Span>> {
        if self.sdk.is_none() {
            let text = documents.get(uri)?.to_string();
            return self.syntactic.references(uri, &text, offset);
        }
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.document_highlights(unit, offset)
    }

    fn implementations(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Vec<(String, Span)> {
        // `searchAllSubtypes` searches every known file, including the whole SDK
        // (`JSString` from `dart:_interceptors` implements `String`): the wide
        // load, like the references'. The library's load names the search.
        let name = self.library(documents, uri).and_then(|project| {
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)
        });
        if let Some(name) = name
            && let Some(wide) = self.wide(documents, uri, &name, false)
            && let Some(unit) = wide.uri_unit(uri)
        {
            return wide
                .implementations(unit, offset)
                .into_iter()
                .filter_map(|(u, s)| Some((wide.unit_uri(u)?, s)))
                .collect();
        }
        let Some(project) = self.project(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project
            .implementations(unit, offset)
            .into_iter()
            .filter_map(|(u, s)| Some((project.unit_uri(u)?, s)))
            .collect()
    }

    fn colors(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, [u8; 4])> {
        // The libraries of the color constants (Flutter's `Colors`) need their
        // bodies: without them inferred, a dedicated load with them.
        let extras = {
            let Some(project) = self.library(documents, uri) else {
                return Vec::new();
            };
            let Some(unit) = project.uri_unit(uri) else {
                return Vec::new();
            };
            project.color_libraries(unit)
        };
        if extras.is_empty() {
            let Some(mut project) = self.library(documents, uri) else {
                return Vec::new();
            };
            let Some(unit) = project.uri_unit(uri) else {
                return Vec::new();
            };
            return project.colors(unit);
        }
        let Some(sdk) = self.sdk.as_ref() else {
            return Vec::new();
        };
        let Some(project) = crate::project::load_library(sdk, documents, uri) else {
            return Vec::new();
        };
        let crate::project::Project {
            query,
            root,
            mut libraries,
            referenced_names,
            reverse_index,
            comments,
            occurrences_by_unit,
            written_subtypes,
        } = project;
        let mut list: Vec<dartforge_elements::model::LibraryId> = libraries
            .iter()
            .copied()
            .chain(extras.iter().copied())
            .collect();
        list.sort();
        list.dedup();
        libraries.extend(extras);
        let mut project = crate::project::Project {
            query: query.reinfer(&list),
            root,
            libraries,
            referenced_names,
            reverse_index,
            comments,
            occurrences_by_unit,
            written_subtypes,
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project.colors(unit)
    }

    fn color_presentation(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<bool> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project
            .has_flutter_color()
            .then(|| project.requires_const(unit, offset))
    }

    fn augmentation_lenses(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        augmented: bool,
        augmentations: bool,
    ) -> Vec<(Span, &'static str, String, Span)> {
        let Some(project) = self.library(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project
            .augmentation_lenses(unit, augmented, augmentations)
            .into_iter()
            .filter_map(|(s, t, u, target)| Some((s, t, project.unit_uri(u)?, target)))
            .collect()
    }

    fn augmentation_neighbor(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        next: bool,
    ) -> Option<(String, Span)> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.augmentation_neighbor(unit, offset, next)?;
        Some((project.unit_uri(u)?, s))
    }

    fn upper(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<(String, Span)> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.upper(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn type_definition(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<(String, Span)> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.type_definition(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn closing_labels(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, String)> {
        let Some(project) = self.library(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project.closing_labels(unit)
    }

    fn hints(&mut self, documents: &DocumentStore, uri: &str) -> Vec<crate::Hint> {
        let Some(project) = self.library(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project.hints(unit)
    }

    fn semantic_tokens(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        multiline: bool,
        range: Option<(usize, usize)>,
    ) -> Option<Vec<u32>> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let highlights = project.highlights(unit);
        let text = documents.get(uri)?;
        let table = documents.lines(uri)?;
        Some(crate::semantic_tokens::encode(
            text,
            table,
            &highlights,
            multiline,
            range,
        ))
    }

    fn prepare_calls(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<crate::CallItem> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let e = project.call_target(unit, offset)?;
        project.call_item(e)
    }

    fn calls(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
        name: &str,
        constructor: bool,
        received: bool,
    ) -> Vec<(crate::CallItem, Vec<Span>)> {
        let Some(project) = self.project(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        let Some((e, _)) = project.item_target(unit, offset, name, constructor) else {
            return Vec::new();
        };
        if received {
            // Only executables receive calls.
            if !matches!(
                e,
                crate::call_hierarchy::CallElem::Function(_)
                    | crate::call_hierarchy::CallElem::Local(..)
            ) {
                return Vec::new();
            }
            project.incoming_calls(e)
        } else {
            project.outgoing_calls(unit, offset)
        }
    }

    fn prepare_hierarchy(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<crate::TypeItem> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (c, ty) = project.hierarchy_target(unit, offset)?;
        project.type_item(c, ty)
    }

    fn hierarchy(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        reference: &str,
        anchor: Option<(&str, &[usize])>,
        supertypes: bool,
    ) -> Option<Vec<crate::TypeItem>> {
        let mut project = self.project(documents, uri)?;
        if supertypes {
            project.reference_supertypes(reference, anchor)
        } else {
            project.reference_subtypes(reference)
        }
    }

    fn set_completion_max(&mut self, max: usize) {
        self.completion_max = max;
    }

    fn set_not_imported_in_completion(&mut self, yes: bool) {
        self.not_imported_in_completion = yes;
    }

    fn completion(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<crate::Completion> {
        let text = documents.get(uri)?;
        let features = self.syntactic.features(uri, text);
        self.sdk.as_ref()?;
        let self_max = self.completion_max;
        let not_imported = self.not_imported_in_completion;
        let SemanticAnalyzer {
            sdk: Some(sdk),
            known,
            session,
            incremental,
            ..
        } = self
        else {
            return None;
        };
        let sdk: &SdkLayout = sdk;
        let indices = crate::completion::Indices { known };
        // The retained state of the library (that of the document's text), for the
        // incremental path (docs/LSP-ESPECIFICACAO.md §16.7); loaded and retained
        // here if missing, so that the following keystrokes find it.
        let file = crate::project::uri_file(uri)?;
        let mut retained = if *incremental {
            session.get(crate::session::Scope::Library(file), documents, || {
                crate::project::load_library(sdk, documents, uri)
            })
        } else {
            None
        };
        crate::completion::completion(
            crate::completion::CompletionInput {
                sdk,
                indices,
                documents,
                uri,
                text,
                offset,
                features,
                max: self_max,
                not_imported,
            },
            retained.as_deref_mut(),
        )
    }
}
