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
    pub(crate) display_reference: crate::query::DisplayReference,
    /// The last loaded program, reused while nothing changed.
    pub(crate) session: crate::session::Session,
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
    /// The resolved-reference index of the active search project, once built
    /// ([`crate::reference_index`]). Switching roots drops it before loading
    /// the next project's cache, retaining at most one decoded index.
    reference_index: Option<crate::reference_index::ReferenceIndex>,
    /// A build of the index running in a child: the project root and the
    /// channel of its encoded index (`None`: it failed).
    index_build: Option<(
        std::path::PathBuf,
        std::sync::mpsc::Receiver<Option<Vec<u8>>>,
    )>,
    /// The roots whose index could not be built (not tried again until the
    /// configuration changes).
    index_failed: std::collections::HashSet<std::path::PathBuf>,
    /// The index's answers from a record's identifier facts and from an
    /// identification in the document's library (the comparison's counts).
    index_answers: (usize, usize),
    /// When the project's files were last compared with the index's records
    /// on disk, and whether the watcher reported a change since: until one of
    /// them asks for it, a search only compares the open documents.
    index_disk_check: Option<std::time::Instant>,
    index_disk_dirty: bool,
    /// The documents whose library's load did not fit in the session's budget:
    /// their records are made again in a child.
    over_budget: std::collections::HashSet<std::path::PathBuf>,
    /// Where the resident's records of the edited files arrive
    /// ([`crate::typed::ResidentRecords`]).
    resident_records: Option<std::sync::Arc<crate::typed::ResidentRecords>>,
    /// The project files whose resident record came with another API than the
    /// index's record had: their dependents are recorded again.
    index_declaration_changes: Vec<std::path::PathBuf>,
    /// Files still requiring records after a failed refresh. An edited
    /// resident record may already be merged, so comparing text alone can no
    /// longer recover its importers on the next request.
    index_refresh_pending: Vec<std::path::PathBuf>,
    /// External records built from unsaved text. Their disk text must be
    /// checked again after closing the corresponding document.
    index_external_overlays: Vec<std::path::PathBuf>,
    /// Bounded previous open texts, used only to prove selective API refreshes.
    index_edits: crate::index_edit::Sources,
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
        let display_reference = sdk.as_ref().map_or(
            crate::query::DisplayReference::default(),
            crate::query::DisplayReference::for_sdk,
        );
        Self {
            display_reference,
            syntactic: SyntacticAnalyzer::new(),
            sdk,
            session: crate::session::Session::new(),
            wide_session: crate::session::Session::wide(),
            wide_fits: Default::default(),
            search_exe: None,
            reference_index: None,
            index_build: None,
            index_failed: Default::default(),
            index_answers: (0, 0),
            index_disk_check: None,
            index_disk_dirty: true,
            over_budget: Default::default(),
            resident_records: None,
            index_declaration_changes: Vec::new(),
            index_refresh_pending: Vec::new(),
            index_external_overlays: Vec::new(),
            index_edits: Default::default(),
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

    /// The resident's answer to the library query `call` on `uri`
    /// ([`crate::remote`]); `None` when it gives none (the caller answers here,
    /// as before). An answer drops a library the session loaded while the
    /// resident could not answer.
    fn remote(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        call: crate::remote::Call,
    ) -> Option<crate::remote::Answer> {
        if !crate::remote::enabled() {
            return None;
        }
        let link = self.resident_records.clone()?;
        let sdk = self.sdk.as_ref()?;
        let file = crate::project::uri_file(uri)?;
        if file.extension().is_none_or(|e| e != "dart") || documents.get(uri).is_none() {
            return None;
        }
        let request = crate::remote::QueryRequest {
            sdk_lib: sdk.root.clone(),
            uri: uri.to_string(),
            documents: Self::document_list(documents),
            call,
        };
        let started = std::time::Instant::now();
        let answer = link.query(&file, &request, crate::remote::wait_limit());
        if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
            eprintln!(
                "[dartforge-lsp] resident query {:?}: {} in {} us",
                request.call,
                match &answer {
                    None => "no answer",
                    Some(crate::remote::Answer::Unavailable) => "unavailable",
                    Some(_) => "answered",
                },
                started.elapsed().as_micros()
            );
        }
        match answer? {
            crate::remote::Answer::Unavailable => None,
            answer => {
                self.session.drop_library();
                Some(answer)
            }
        }
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
            && let Some(h) = project.constructor_hover(unit, offset).or_else(|| {
                project
                    .identify(unit, offset)
                    .ok()
                    .flatten()
                    .and_then(|d| project.hover(unit, &d))
            })
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
        // The index first: it needs no load of the document's library when the
        // document's record knows what is at the word.
        if let Ok(found) = self.indexed_references(documents, uri, offset) {
            return found;
        }
        self.unindexed_references(documents, uri, offset)
    }

    /// The search without the index (and the comparison's reference).
    fn unindexed_references(
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
        let (name, parameter) = {
            let project = self.library(documents, uri)?;
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            let parameter = match d.target {
                crate::project::Target::Local { unit, declaration } => {
                    crate::project::parameter_in(&project.program().unit(unit).ast, declaration)
                        .is_some()
                }
                _ => false,
            };
            (project.search_name(&d.target), parameter)
        };
        let Some(name) = name else {
            // A parameter's search reaches the calls of other files (the labels of
            // a named one, the arguments of an optional positional one): the whole
            // project. A local, a type parameter or a prefix: only this file has
            // them.
            if parameter {
                let project = self.project(documents, uri)?;
                return references_in(&project, uri, offset);
            }
            let library = self.library(documents, uri)?;
            return references_in(&library, uri, offset);
        };
        // A private name is only visible in its library, whose files are the
        // only candidates (`_addResults`): the library's load has them all.
        if name.starts_with('_') {
            let library = self.library(documents, uri)?;
            return references_in(&library, uri, offset);
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
        if !self.ready_index(documents, uri) {
            return Err(Miss::Unsupported("no index ready"));
        }
        let mut index = self
            .reference_index
            .take()
            .ok_or(Miss::Unsupported("no index"))?;
        let answer = (|| {
            // What the record of the document says is at the word, or, without a
            // fact, what the document's library says.
            let text = documents.get(uri).ok_or(Miss::Unsupported("no document"))?;
            let Some(word) = crate::project::word(text, offset) else {
                return Ok(None);
            };
            let path = crate::project::uri_file(uri)
                .map(|f| dartforge_elements::generated::key(&f))
                .ok_or(Miss::Unsupported("no file"))?;
            let fact = index.fact_at(&path, word.start).cloned();
            if fact.is_some() {
                self.index_answers.0 += 1;
            }
            let found = match fact {
                Some(crate::reference_index::IdentFact::Target(t)) => index.references(&t)?,
                Some(crate::reference_index::IdentFact::Local {
                    declaration,
                    uses,
                    positional,
                }) => index
                    .local_answer(&path, declaration, &uses, positional)
                    .ok_or(Miss::Unsupported("no record"))?,
                None => {
                    self.index_answers.1 += 1;
                    let library = self
                        .library(documents, uri)
                        .ok_or(Miss::Unsupported("no library"))?;
                    let unit = library.uri_unit(uri).ok_or(Miss::Unsupported("no unit"))?;
                    let Ok(Some(d)) = library.identify(unit, offset) else {
                        return Ok(None);
                    };
                    let mut tables = crate::reference_index::Tables {
                        keys: &mut index.keys,
                        words: &mut index.words,
                        paths: &mut index.paths,
                    };
                    let t = crate::reference_index::target_of(&library, unit, &mut tables, &d)
                        .ok_or(Miss::Unsupported("a local search"))?;
                    index.references(&t)?
                }
            };
            let (declaration, uses) = found;
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

    /// The implementations from the index: the pivot of the document's record
    /// (or of the document's library), its subtypes over the class facts.
    fn indexed_implementations(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        offset: usize,
    ) -> Option<Vec<(String, Span)>> {
        if !self.ready_index(documents, uri) {
            return None;
        }
        let mut index = self.reference_index.take()?;
        let answer = (|| {
            let text = documents.get(uri)?;
            let word = crate::project::word(text, offset)?;
            let path = dartforge_elements::generated::key(&crate::project::uri_file(uri)?);
            let target = match index.fact_at(&path, word.start).cloned() {
                Some(crate::reference_index::IdentFact::Target(t)) => t,
                Some(crate::reference_index::IdentFact::Local { .. }) => return Some(Vec::new()),
                None => {
                    let library = self.library(documents, uri)?;
                    let unit = library.uri_unit(uri)?;
                    let d = library.identify(unit, offset).ok()??;
                    let mut tables = crate::reference_index::Tables {
                        keys: &mut index.keys,
                        words: &mut index.words,
                        paths: &mut index.paths,
                    };
                    crate::reference_index::target_of(&library, unit, &mut tables, &d)?
                }
            };
            if target.pivot.is_none() {
                return Some(Vec::new());
            }
            index.implementations(&target)
        })();
        self.reference_index = Some(index);
        answer
    }

    /// Builds the reference index of the project of `uri` in this process
    /// (the comparison of [`SemanticAnalyzer::check_reference_index`]).
    fn build_reference_index_here(&mut self, documents: &DocumentStore, uri: &str) -> bool {
        let Some(sdk) = self.sdk.as_ref() else {
            return false;
        };
        let index = crate::reference_index::build(sdk, documents, uri);
        self.replace_reference_index(index);
        self.reference_index.is_some()
    }

    /// Compares, at every identifier of the open documents `uris`, the
    /// references the index gives with those of the search without it (one
    /// position per target and file). The report has the counts and the first
    /// differences.
    #[doc(hidden)]
    pub fn check_reference_index(&mut self, documents: &DocumentStore, uris: &[String]) -> String {
        let Some(first) = uris.first() else {
            return String::new();
        };
        let started = std::time::Instant::now();
        if !self.build_reference_index_here(documents, first) {
            return "the index could not be built".to_string();
        }
        let mut built = format!("index built in {} ms\n", started.elapsed().as_millis());
        // `DARTFORGE_INDEX_CHECK_RESIDENT=1`: the records of the files come from
        // the resident's analyses instead (docs/LSP.md, "Décima sexta rodada").
        if std::env::var_os("DARTFORGE_INDEX_CHECK_RESIDENT").is_some_and(|v| v == "1") {
            built.push_str(&self.record_from_resident(documents, uris));
        }
        built + &self.compare_with_current_index(documents, uris)
    }

    /// Records each of `uris` again from an analysis of a resident in this
    /// process (as the typed worker's resident process does after an edit),
    /// and puts the records in the index. Each file is analyzed twice: with a
    /// body edit, then with its text, so that the second record reuses what
    /// the first loan computed of the units that did not change.
    fn record_from_resident(&mut self, documents: &DocumentStore, uris: &[String]) -> String {
        use dartforge_elements::generated::key;
        let (Some(sdk), Some(index)) = (self.sdk.as_ref(), self.reference_index.as_mut()) else {
            return "no index to record from the resident\n".to_string();
        };
        let Ok(engine) = crate::typed::lsp_engine(&sdk.root) else {
            return "the engine could not start\n".to_string();
        };
        let mut resident = dartforge_parity::analysis::Resident::new();
        resident.keep_bodies(true);
        let mut cache = crate::reference_index::ResidentCache::default();
        let patch_keys = crate::reference_index::analyzer_view_has_patches(&sdk.root);
        let sdk_key = key(&sdk.root);
        let mut texts: std::collections::HashMap<std::path::PathBuf, String> = documents
            .uris()
            .filter_map(|u| {
                Some((
                    key(&crate::project::uri_file(u)?),
                    documents.get(u)?.to_string(),
                ))
            })
            .collect();
        let (mut recorded, mut failed) = (0usize, 0usize);
        for uri in uris {
            let Some(file) = crate::project::uri_file(uri) else {
                continue;
            };
            let k = key(&file);
            let Some(text) = texts.get(&k).cloned() else {
                continue;
            };
            let root = crate::project::project_root(&file);
            let packages = root.join(".dart_tool").join("package_config.json");
            let packages = packages.is_file().then_some(packages);
            let mut made = None;
            let passes = if std::env::var_os("DARTFORGE_INDEX_CHECK_ONE_PASS").is_some() {
                vec![text.clone()]
            } else {
                vec![format!("{text}\n"), text.clone()]
            };
            for edited in passes {
                texts.insert(k.clone(), edited);
                let analysis = engine.analyze_resident(
                    &mut resident,
                    &root,
                    std::slice::from_ref(&file),
                    packages.as_deref(),
                    &texts,
                    &|| false,
                );
                cache.analyzed();
                made = analysis.and_then(|_| {
                    resident
                        .lend_analysis(|lent| {
                            crate::reference_index::records_of_analysis(
                                lent,
                                std::slice::from_ref(&k),
                                &mut cache,
                                patch_keys,
                            )
                        })
                        .flatten()
                });
            }
            match made.and_then(|b| crate::reference_index::ReferenceIndex::decode(&b)) {
                Some(mut part) if part.file(&k).is_some() => {
                    index.adopt_sdk_locations(&mut part, &sdk_key);
                    if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
                        let (old, new) = (index.describe(&k), part.describe(&k));
                        let only_old: Vec<&String> = old.difference(&new).take(12).collect();
                        let only_new: Vec<&String> = new.difference(&old).take(12).collect();
                        if !only_old.is_empty() || !only_new.is_empty() {
                            eprintln!(
                                "[dartforge-lsp] record of {}: only the index's {only_old:?}; only the resident's {only_new:?}",
                                k.display()
                            );
                        }
                    }
                    index.merge(part);
                    recorded += 1;
                }
                _ => failed += 1,
            }
        }
        format!(
            "recorded from the resident: {recorded} files ({failed} without a record), {} loans reused the cache\n",
            cache.reused
        )
    }

    /// [`SemanticAnalyzer::check_reference_index`] with the index this
    /// analyzer has (refreshed before each answer), without building it.
    /// `DARTFORGE_INDEX_CHECK_DUMP=<offset>` adds both answers at that offset
    /// to the report.
    #[doc(hidden)]
    pub fn compare_with_current_index(
        &mut self,
        documents: &DocumentStore,
        uris: &[String],
    ) -> String {
        use std::fmt::Write as _;
        type Answer = Option<(Option<(String, Span)>, Vec<(String, Span)>)>;
        let mut report = String::new();
        // The search without the index is compared with the whole project loaded
        // (the session then serves the library queries from it): a library's load
        // misses what other files' bodies say of a local, as an optional positional
        // parameter's empty references in the calls.
        if let Some(first) = uris.first() {
            let _ = self.project(documents, first);
        }
        let mut compared = 0usize;
        let mut equal = 0usize;
        // Equal, not answered by the index, different.
        let mut implementations = (0usize, 0usize, 0usize);
        let mut prepares = (0usize, 0usize, 0usize);
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
                    format!("{:?}", d.target)
                };
                if !seen.insert(target.clone()) {
                    continue;
                }
                let t = std::time::Instant::now();
                let new = self.indexed_references(documents, uri, offset);
                new_time += t.elapsed();
                let new = match new {
                    Ok(n) => n,
                    Err(miss) => {
                        let reason = match miss {
                            crate::reference_index::Miss::Unsupported(r) => r.to_string(),
                            crate::reference_index::Miss::Key(k) => format!("a key: {k}"),
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
                if std::env::var("DARTFORGE_INDEX_CHECK_DUMP")
                    .is_ok_and(|o| o == offset.to_string())
                {
                    let _ = writeln!(
                        report,
                        "dump {uri}:{offset} {target}\nindex {new:?}\nsearch {old:?}"
                    );
                }
                let sorted = |mut v: Vec<(String, Span)>| {
                    v.sort_by(|a, b| {
                        (a.0.as_str(), a.1.start, a.1.end).cmp(&(b.0.as_str(), b.1.start, b.1.end))
                    });
                    v
                };
                let t = std::time::Instant::now();
                let new_impl = self
                    .indexed_implementations(documents, uri, offset)
                    .map(sorted);
                new_time += t.elapsed();
                let index = self.reference_index.take();
                let t = std::time::Instant::now();
                let old_impl = sorted(self.unindexed_implementations(documents, uri, offset));
                old_time += t.elapsed();
                self.reference_index = index;
                // `prepareRename`: the record's answer and the library's.
                let fact = documents.get(uri).and_then(|text| {
                    let word = crate::project::word(text, offset)?;
                    let path = dartforge_elements::generated::key(&crate::project::uri_file(uri)?);
                    self.reference_index
                        .as_ref()?
                        .prepare_at(&path, word.start)
                        .cloned()
                });
                let library_answer = self
                    .library(documents, uri)
                    .map(|l| crate::rename::prepare(&l, uri, offset));
                match (fact, library_answer) {
                    (None, _) | (_, None) => prepares.1 += 1,
                    (Some(f), Some(answer)) => {
                        let same = match (&f, &answer) {
                            (crate::reference_index::PrepareFact::Nothing, Ok(None)) => true,
                            (
                                crate::reference_index::PrepareFact::Range { start, end, old },
                                Ok(Some((s, o))),
                            ) => *start as usize == s.start && *end as usize == s.end && old == o,
                            (crate::reference_index::PrepareFact::Refused(m), Err(e)) => m == e,
                            _ => false,
                        };
                        if same {
                            prepares.0 += 1;
                        } else {
                            prepares.2 += 1;
                            if differences.len() < 30 {
                                differences.push(format!(
                                    "{uri}:{offset} {target}: prepareRename, index {f:?} library {answer:?}"
                                ));
                            }
                        }
                    }
                }
                match new_impl {
                    None => implementations.1 += 1,
                    Some(n) if n == old_impl => implementations.0 += 1,
                    Some(n) => {
                        implementations.2 += 1;
                        if differences.len() < 30 {
                            differences.push(format!(
                                "{uri}:{offset} {target}: implementations, index {} search {}",
                                n.len(),
                                old_impl.len()
                            ));
                        }
                    }
                }
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
        let _ = writeln!(
            report,
            "index answers from the identifier facts {}, from the library {}",
            self.index_answers.0, self.index_answers.1
        );
        let _ = writeln!(
            report,
            "implementations: equal {}, not answered {}, different {}",
            implementations.0, implementations.1, implementations.2
        );
        let _ = writeln!(
            report,
            "prepareRename: equal {}, not answered {}, different {}",
            prepares.0, prepares.1, prepares.2
        );
        for (reason, n) in misses {
            let _ = writeln!(report, "not answered ({reason}): {n}");
        }
        for d in differences {
            let _ = writeln!(report, "{d}");
        }
        report
    }
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
pub(crate) fn load_sdk(lib: &std::path::Path) -> Result<SdkLayout, String> {
    let sdk = SdkLayout::load_as_analyzer(lib)?;
    let core = sdk
        .library("core")
        .ok_or_else(|| "it declares no dart:core library".to_string())?;
    std::fs::read_to_string(&core.path)
        .map_err(|e| format!("could not read {}: {e}", core.path.display()))?;
    Ok(sdk)
}

/// The references of what is at `offset` of `uri`, in the given project.
pub(crate) fn references_in(
    project: &crate::project::Project,
    uri: &str,
    offset: usize,
) -> Option<References> {
    references_at(project, project.uri_unit(uri)?, offset)
}

/// [`references_in`] at `offset` of `unit`.
pub(crate) fn references_at(
    project: &crate::project::Project,
    unit: UnitId,
    offset: usize,
) -> Option<References> {
    let d = project.identify(unit, offset).ok()??;
    references_identified(project, unit, offset, &d)
}

/// [`references_at`] with the resolution already obtained from this project's
/// current state at the same unit and offset (the reference index records
/// several facts at each identifier).
pub(crate) fn references_identified(
    project: &crate::project::Project,
    unit: UnitId,
    offset: usize,
    d: &crate::project::Denoted,
) -> Option<References> {
    {
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
        let declaration = project.declaration_to_references(d);
        let declarations = project.declarations(&d.target);
        // Non-local search: only the candidate files of `_addResults`
        // (docs/LSP-ESPECIFICACAO.md §11.2), including Dart's gaps (factory
        // redirection, operators outside the library).
        let candidates = project.search_name(&d.target).map(|name| {
            let mut units: Vec<UnitId> = declarations.iter().map(|(u, _, _)| *u).collect();
            if let Some((u, _)) = project.declaration(d) {
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

    fn symbols_in(&mut self, documents: &DocumentStore, uri: &str) -> Vec<serde_json::Value> {
        let Some(text) = documents.get(uri) else {
            return Vec::new();
        };
        // A declaration without annotations cannot acquire deprecation from
        // an owner's annotation. Keep ordinary outlines on the parse-only path.
        if !text.contains('@') {
            return self.syntactic.symbols(uri, text);
        }
        if let Some(crate::remote::Answer::Symbols(symbols)) =
            self.remote(documents, uri, crate::remote::Call::Symbols)
        {
            return symbols;
        }
        if let Some(library) = self.library(documents, uri)
            && let Some(unit) = library.uri_unit(uri)
        {
            return crate::symbol_metadata::for_project(&library, unit);
        }
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
        if let Some(crate::remote::Answer::Definition(d)) =
            self.remote(documents, uri, crate::remote::Call::Definition { offset })
        {
            return d;
        }
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
        if let Some(crate::remote::Answer::Hover(h)) =
            self.remote(documents, uri, crate::remote::Call::Hover { offset })
        {
            return h;
        }
        self.pass_hover(uri, text, offset, documents)
    }

    fn closed_document(&mut self, uri: &str) {
        if let Some(path) = crate::project::uri_file(uri) {
            self.index_edits
                .remove(&dartforge_elements::generated::key(&path));
        }
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

    fn resident_records(&mut self, records: std::sync::Arc<crate::typed::ResidentRecords>) {
        self.resident_records = Some(records);
    }

    fn disk_changed(&mut self, paths: &[std::path::PathBuf]) {
        if !paths.is_empty() {
            self.index_disk_dirty = true;
            if let Some(records) = &self.resident_records {
                records.invalidate();
            }
            self.session.disk_changed();
            self.wide_session.disk_changed();
        }
    }

    fn watched_files(&mut self, yes: bool) {
        self.session.trust_watcher(yes);
        self.wide_session.trust_watcher(yes);
    }

    fn project_configuration_changed(&mut self) {
        self.replace_reference_index(None);
        self.index_build = None;
        self.index_failed.clear();
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
        if let Some(path) = crate::project::uri_file(uri) {
            self.index_edits
                .remove(&dartforge_elements::generated::key(&path));
        }
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
        // Right after an edit the document's record is still being made, while
        // the session already holds the library with the new text (the edit
        // swapped its unit in place): it answers at once.
        let current_record = |s: &Self| {
            let (Some(index), Some(text), Some(path)) = (
                s.reference_index.as_ref(),
                documents.get(uri),
                crate::project::uri_file(uri),
            ) else {
                return false;
            };
            index
                .file(&dartforge_elements::generated::key(&path))
                .is_some_and(|r| {
                    r.length as usize == text.len()
                        && r.stamp == crate::reference_index::text_stamp(text)
                })
        };
        if !current_record(self)
            && crate::project::uri_file(uri).is_some_and(|f| self.session.holds_library(&f))
            && let Some(project) = self.library(documents, uri)
        {
            return crate::rename::prepare(&project, uri, offset);
        }
        // The resident has the library of the new text before the record it
        // makes of it: it answers without waiting for the record.
        if !current_record(self)
            && let Some(crate::remote::Answer::Prepare(r)) = self.remote(
                documents,
                uri,
                crate::remote::Call::PrepareRename { offset },
            )
        {
            return r;
        }
        // The answer the document's record keeps, when it is of the text.
        if self.ready_index(documents, uri)
            && let Some(index) = &self.reference_index
            && let Some(text) = documents.get(uri)
            && let Some(word) = crate::project::word(text, offset)
            && let Some(path) = crate::project::uri_file(uri)
            && let Some(fact) =
                index.prepare_at(&dartforge_elements::generated::key(&path), word.start)
        {
            return match fact {
                crate::reference_index::PrepareFact::Nothing => Ok(None),
                crate::reference_index::PrepareFact::Range { start, end, old } => Ok(Some((
                    Span {
                        start: *start as usize,
                        end: *end as usize,
                    },
                    old.clone(),
                ))),
                crate::reference_index::PrepareFact::Refused(m) => Err(m.clone()),
            };
        }
        // `getElementToRename` and `checkInitialConditions` look only at the
        // element under the cursor: the library's load is enough (the rename
        // itself loads the whole project).
        if let Some(crate::remote::Answer::Prepare(r)) = self.remote(
            documents,
            uri,
            crate::remote::Call::PrepareRename { offset },
        ) {
            return r;
        }
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
        // The whole answer from the resident, which has the library.
        if let Some(crate::remote::Answer::Actions(actions)) = self.remote(
            documents,
            uri,
            crate::remote::Call::Actions {
                start,
                end,
                published: published_.to_vec(),
            },
        ) {
            return actions;
        }
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
        if let Some(crate::remote::Answer::Signature(r)) = self.remote(
            documents,
            uri,
            crate::remote::Call::Signature { offset, auto },
        ) {
            return r;
        }
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
        if let Some(crate::remote::Answer::Spans(r)) =
            self.remote(documents, uri, crate::remote::Call::Highlights { offset })
        {
            return r;
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
        if let Some(found) = self.indexed_implementations(documents, uri, offset) {
            return found;
        }
        self.unindexed_implementations(documents, uri, offset)
    }

    fn colors(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, [u8; 4])> {
        if let Some(crate::remote::Answer::Colors(r)) =
            self.remote(documents, uri, crate::remote::Call::Colors)
        {
            return r;
        }
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
            patch_keys,
            unit_paths,
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
            patch_keys,
            unit_paths,
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
        if let Some(crate::remote::Answer::Flag(r)) = self.remote(
            documents,
            uri,
            crate::remote::Call::ColorPresentation { offset },
        ) {
            return r;
        }
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
        if let Some(crate::remote::Answer::Location(r)) =
            self.remote(documents, uri, crate::remote::Call::Upper { offset })
        {
            return r;
        }
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
        if let Some(crate::remote::Answer::Location(r)) = self.remote(
            documents,
            uri,
            crate::remote::Call::TypeDefinition { offset },
        ) {
            return r;
        }
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.type_definition(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn closing_labels(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, String)> {
        if let Some(crate::remote::Answer::Labels(r)) =
            self.remote(documents, uri, crate::remote::Call::ClosingLabels)
        {
            return r;
        }
        let Some(project) = self.library(documents, uri) else {
            return Vec::new();
        };
        let Some(unit) = project.uri_unit(uri) else {
            return Vec::new();
        };
        project.closing_labels(unit)
    }

    fn hints(&mut self, documents: &DocumentStore, uri: &str) -> Vec<crate::Hint> {
        if let Some(crate::remote::Answer::Hints(r)) =
            self.remote(documents, uri, crate::remote::Call::Hints)
        {
            return r;
        }
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
        if let Some(crate::remote::Answer::Tokens(r)) = self.remote(
            documents,
            uri,
            crate::remote::Call::SemanticTokens { multiline, range },
        ) {
            return r;
        }
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
        if let Some(crate::remote::Answer::CallItem(r)) =
            self.remote(documents, uri, crate::remote::Call::PrepareCalls { offset })
        {
            return r;
        }
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
        if let Some(crate::remote::Answer::TypeItem(r)) = self.remote(
            documents,
            uri,
            crate::remote::Call::PrepareHierarchy { offset },
        ) {
            return r;
        }
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
        documents.get(uri)?;
        self.sdk.as_ref()?;
        if let Some(crate::remote::Answer::Completion(c)) = self.remote(
            documents,
            uri,
            crate::remote::Call::Completion {
                offset,
                max: self.completion_max,
                not_imported: self.not_imported_in_completion,
            },
        ) {
            return c;
        }
        let text = documents.get(uri)?;
        let features = self.syntactic.features(uri, text);
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

/// The hidden argument that makes the `dartforge-lsp` binary a reference
/// index process ([`reference_index_child`]).
pub const REFERENCE_INDEX_ARGUMENT: &str = "--reference-index";

/// What the server asks the index child: the whole index of the project of
/// `uri` (`files` absent), or the records of the libraries of `files`.
#[derive(serde::Serialize, serde::Deserialize)]
struct IndexRequest {
    sdk_lib: std::path::PathBuf,
    uri: String,
    documents: Vec<(String, i32, String)>,
    files: Option<Vec<std::path::PathBuf>>,
}

/// The reference index child process ([`REFERENCE_INDEX_ARGUMENT`]): reads an
/// `IndexRequest` from `stdin`, builds and writes the encoded index
/// (`crate::reference_index::ReferenceIndex::encode`) to `stdout`. Its
/// program (the wide load) never stays in the server's memory.
pub fn reference_index_child() -> i32 {
    use std::io::{Read, Write};
    crate::reference_index::watch_parent();
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        return 2;
    }
    let Ok(request) = serde_json::from_slice::<IndexRequest>(&input) else {
        return 2;
    };
    let Ok(sdk) = load_sdk(&request.sdk_lib) else {
        return 2;
    };
    let mut documents = DocumentStore::new();
    for (uri, version, text) in request.documents {
        documents.open(uri, version, text);
    }
    let index = match &request.files {
        None => crate::reference_index::build(&sdk, &documents, &request.uri),
        Some(files) => crate::reference_index::build_files(&sdk, &documents, &request.uri, files),
    };
    let Some(index) = index else {
        return 1;
    };
    match std::io::stdout().lock().write_all(&index.encode()) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Runs `request` in an index child of `exe`: its bytes, or `None` when the
/// child failed.
fn run_index_child(exe: &std::path::Path, request: &IndexRequest) -> Option<Vec<u8>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(exe)
        .arg(REFERENCE_INDEX_ARGUMENT)
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
    output.status.success().then_some(output.stdout)
}

/// Where the index of `root` is kept between sessions (the user's cache).
fn index_cache_path(root: &std::path::Path, sdk: &std::path::Path) -> Option<std::path::PathBuf> {
    let key = format!("{}\n{}", root.display(), sdk.display());
    let name = format!(
        "reference-index-{:016x}.bin",
        crate::reference_index::text_stamp(&key)
    );
    Some(
        dartforge_elements::distribution::user_cache()?
            .join("lsp")
            .join(name),
    )
}

/// The index of `root` kept by an earlier session, when its SDK and package
/// records match the current disk or overlays. Unchanged dates and sizes
/// avoid reading disk-backed text; project files are refreshed before search.
fn load_cached_index(
    root: &std::path::Path,
    sdk: &std::path::Path,
    documents: &DocumentStore,
    entry: &std::path::Path,
) -> Option<crate::reference_index::ReferenceIndex> {
    let bytes = crate::memory::file_bytes(&index_cache_path(root, sdk)?)?;
    let index = crate::reference_index::ReferenceIndex::decode(&bytes)?;
    if index.root != root || !index.package_configuration_current(entry) {
        return None;
    }
    (!external_records_changed(&index, documents, &[], &[], true)).then_some(index)
}

/// How long a search waits for the resident's record of an edited document
/// being made, before recording it here.
const RECORD_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// The text of a record's file now, when it is not the record's: the open
/// document's, or the disk's. `Some(None)`: the file is gone.
fn changed_text(
    r: &crate::reference_index::FileRecord,
    documents: &DocumentStore,
) -> Option<Option<String>> {
    let open = url::Url::from_file_path(&r.path)
        .ok()
        .and_then(|u| documents.get(u.as_str()).map(str::to_string));
    let text = match open {
        Some(t) => t,
        None => {
            if r.disk {
                let (modified, length) = crate::reference_index::file_date(&r.path);
                if modified == r.modified && length == Some(u64::from(r.length)) {
                    return None;
                }
            }
            match std::fs::read_to_string(&r.path) {
                Ok(t) => t,
                Err(_) => return Some(None),
            }
        }
    };
    let same =
        text.len() as u32 == r.length && crate::reference_index::text_stamp(&text) == r.stamp;
    (!same).then_some(Some(text))
}

/// External files are normally frozen, but another open project can edit
/// them. A changed dependency requires a fresh complete index: refreshing
/// just the active project's records would leave foreign declarations and
/// transitive dependency facts resolved against the former text.
fn external_records_changed(
    index: &crate::reference_index::ReferenceIndex,
    documents: &DocumentStore,
    open_files: &[std::path::PathBuf],
    external_overlays: &[std::path::PathBuf],
    check_disk: bool,
) -> bool {
    if check_disk {
        return index
            .records()
            .any(|r| r.frozen && changed_text(r, documents).is_some());
    }
    open_files.iter().chain(external_overlays).any(|path| {
        index
            .file(dartforge_elements::generated::key_of(path).as_ref())
            .is_some_and(|r| r.frozen && changed_text(r, documents).is_some())
    })
}

impl SemanticAnalyzer {
    /// The documents as the children receive them.
    fn document_list(documents: &DocumentStore) -> Vec<(String, i32, String)> {
        documents
            .uris()
            .filter_map(|u| {
                Some((
                    u.to_string(),
                    documents.version(u).unwrap_or(0),
                    documents.get(u)?.to_string(),
                ))
            })
            .collect()
    }

    /// Replaces the complete active index, resetting refresh state belonging
    /// to its former root. A returning project must check closed files on
    /// disk even when another project's last scan is recent.
    fn replace_reference_index(&mut self, index: Option<crate::reference_index::ReferenceIndex>) {
        if let Some(records) = &self.resident_records {
            records.want(index.as_ref().map(|index| index.root.clone()));
        }
        self.index_external_overlays = index
            .iter()
            .flat_map(|index| index.records())
            .filter(|r| r.frozen && !r.disk)
            .map(|r| r.path.clone())
            .collect();
        self.reference_index = index;
        self.index_edits.clear();
        self.index_declaration_changes.clear();
        self.index_refresh_pending.clear();
        self.index_disk_check = None;
        self.index_disk_dirty = true;
    }

    /// The reference index of the project of `uri`, ready and of the current
    /// texts: the built one (a build that finished, or the cache of an earlier
    /// session) with its stale project files recorded again. Without one, a
    /// build starts in a child and the search goes on without it.
    fn ready_index(&mut self, documents: &DocumentStore, uri: &str) -> bool {
        let Some(file) = crate::project::uri_file(uri) else {
            return false;
        };
        let root = crate::project::project_root(&file);
        if self
            .reference_index
            .as_ref()
            .is_some_and(|i| i.root != root)
        {
            self.replace_reference_index(None);
        }
        let completed = self.index_build.as_ref().and_then(|(building, receiver)| {
            use std::sync::mpsc::TryRecvError;
            match receiver.try_recv() {
                Ok(bytes) => Some((building.clone(), bytes)),
                Err(TryRecvError::Disconnected) => Some((building.clone(), None)),
                Err(TryRecvError::Empty) => None,
            }
        });
        if let Some((building, bytes)) = completed {
            self.index_build = None;
            if bytes.is_none() {
                self.index_failed.insert(building);
            } else if building == root && self.reference_index.is_none() {
                match bytes
                    .and_then(|b| crate::reference_index::ReferenceIndex::decode(&b))
                    .filter(|index| index.root == root)
                {
                    Some(index) => self.replace_reference_index(Some(index)),
                    None => {
                        self.index_failed.insert(building);
                    }
                }
            }
            // A completed build of another root already saved its cache.
            // Do not decode it or replace the current project's live index.
        }
        // Reuse this canonical snapshot in the refresh below. Resolving the
        // same URI twice repeats filesystem work on every indexed request.
        let open_files: Vec<_> = documents
            .uris()
            .filter_map(crate::project::uri_file)
            .collect();
        let whole = self.index_disk_dirty
            || self
                .index_disk_check
                .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(30));
        if whole
            && self
                .reference_index
                .as_ref()
                .is_some_and(|index| !index.package_configuration_current(&file))
        {
            // A periodic check can discover a package-map change even without
            // a watcher. Invalidate the semantic sessions as well as the index.
            self.project_configuration_changed();
        }
        if self.reference_index.as_ref().is_some_and(|index| {
            external_records_changed(
                index,
                documents,
                &open_files,
                &self.index_external_overlays,
                whole,
            )
        }) {
            self.replace_reference_index(None);
            if self.search_exe.is_none() {
                let rebuilt = self
                    .sdk
                    .as_ref()
                    .and_then(|sdk| crate::reference_index::build(sdk, documents, uri));
                self.replace_reference_index(rebuilt);
            }
        }
        if self.reference_index.is_none() {
            let Some(sdk) = self.sdk.as_ref() else {
                return false;
            };
            crate::memory::log("before the index's cache");
            if let Some(index) = load_cached_index(&root, &sdk.root, documents, &file) {
                if crate::memory::logging() {
                    let census: Vec<String> = index
                        .census()
                        .iter()
                        .map(|(n, b)| format!("{n} {:.1}", *b as f64 / 1048576.0))
                        .collect();
                    eprintln!("[dartforge-lsp] index census (MiB): {}", census.join(", "));
                }
                self.replace_reference_index(Some(index));
                crate::memory::log("after the index's cache");
            } else {
                let Some(exe) = self.search_exe.clone() else {
                    return false;
                };
                if self.index_build.is_some() || self.index_failed.contains(&root) {
                    return false;
                }
                let request = IndexRequest {
                    sdk_lib: sdk.root.clone(),
                    uri: uri.to_string(),
                    documents: Self::document_list(documents),
                    files: None,
                };
                let cache = index_cache_path(&root, &sdk.root);
                let (sender, receiver) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let bytes = run_index_child(&exe, &request);
                    if let (Some(b), Some(c)) = (&bytes, cache) {
                        let _ = c.parent().map(std::fs::create_dir_all);
                        let _ = std::fs::write(c, b);
                    }
                    let _ = sender.send(bytes);
                });
                self.index_build = Some((root, receiver));
                return false;
            }
        }
        if let Some(r) = &self.resident_records {
            r.want(Some(root));
        }
        self.refresh_index(documents, uri, &open_files, whole || self.index_disk_dirty)
    }

    /// Puts the resident's records of the edited files into `index`, waiting
    /// (up to [`RECORD_WAIT`]) for one being made of an open document's
    /// current text whose record in `index` is not of that text.
    fn absorb_resident_records(
        &mut self,
        documents: &DocumentStore,
        index: &mut crate::reference_index::ReferenceIndex,
        changes: &mut Vec<crate::index_edit::DeclarationChange>,
    ) -> std::collections::HashSet<std::path::PathBuf> {
        let mut fresh = std::collections::HashSet::new();
        let Some(records) = self.resident_records.clone() else {
            return fresh;
        };
        let started = std::time::Instant::now();
        let deadline = started + RECORD_WAIT;
        let mut state = records.lock();
        if state.ready.is_empty() && state.pending.is_empty() {
            return fresh;
        }
        let current = self
            .sdk
            .as_ref()
            .and_then(|sdk| crate::typed::RecordContext::current(&sdk.root, documents));
        loop {
            for batch in std::mem::take(&mut state.ready) {
                if Some(&batch.context) != current.as_ref() {
                    if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
                        eprintln!(
                            "[dartforge-lsp] reference index: discarded superseded resident records"
                        );
                    }
                    continue;
                }
                let bytes = batch.bytes;
                if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
                    eprintln!(
                        "[dartforge-lsp] reference index: resident records ({} bytes) after {} us",
                        bytes.len(),
                        started.elapsed().as_micros()
                    );
                }
                let Some(mut part) = crate::reference_index::ReferenceIndex::decode(&bytes) else {
                    continue;
                };
                if part.root != index.root {
                    continue;
                }
                if let Some(sdk) = &self.sdk {
                    index.adopt_sdk_locations(
                        &mut part,
                        &dartforge_elements::generated::key(&sdk.root),
                    );
                }
                for r in part.records() {
                    if index.file(&r.path).is_none_or(|old| {
                        old.stamp != r.stamp || old.api != r.api || r.api.is_none()
                    }) {
                        self.index_declaration_changes.push(r.path.clone());
                        if self.index_refresh_pending.is_empty()
                            && let Some(old) = index.file(&r.path)
                            && let Some(text) = documents.get(&r.uri)
                            && let Some(change) = self.index_edits.change(old, text)
                        {
                            changes.push(change);
                        }
                    }
                    self.index_edits.remember(r, documents);
                    fresh.insert(r.path.clone());
                }
                index.merge(part);
            }
            let waiting = state.pending.iter().any(|(path, (stamp, id))| {
                if current.is_none() || state.contexts.get(id) != current.as_ref() {
                    return false;
                }
                let current = url::Url::from_file_path(path).ok().and_then(|u| {
                    documents
                        .get(u.as_str())
                        .map(crate::reference_index::text_stamp)
                });
                current == Some(*stamp) && index.file(path).is_none_or(|r| r.stamp != *stamp)
            });
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if !waiting || left.is_zero() {
                return fresh;
            }
            state = records.wait(state, left);
        }
    }

    /// Refreshes complete libraries covering `again`, reporting which records
    /// already contain current foreign positions. Failure leaves the caller's
    /// full pending set in place, including any records merged earlier.
    fn record_index_files(
        &mut self,
        index: &mut crate::reference_index::ReferenceIndex,
        documents: &DocumentStore,
        uri: &str,
        again: &[std::path::PathBuf],
    ) -> Option<std::collections::HashSet<std::path::PathBuf>> {
        use dartforge_elements::generated::key;
        let root = index.root.clone();
        let mut fresh = std::collections::HashSet::new();
        // The document's library, from the session when it covers them all: the
        // session keeps it for the next edits. A library that does not fit in the
        // session's budget is recorded in a child instead (a load here would leave
        // its pages with this process's allocator).
        let mut done = false;
        let mut transient = false;
        let file = crate::project::uri_file(uri);
        let held = file.as_ref().is_some_and(|f| self.session.holds_library(f));
        // The resident's program has the document's library (and usually the
        // rest of the package): it records them without a load here.
        if !held
            && let Some(crate::remote::Answer::Records(bytes)) = self.remote(
                documents,
                uri,
                crate::remote::Call::Records {
                    files: again.to_vec(),
                },
            )
            && let Some(mut part) = crate::reference_index::ReferenceIndex::decode(&bytes)
            && again.iter().all(|a| part.file(a).is_some())
        {
            if let Some(sdk) = &self.sdk {
                index.adopt_sdk_locations(&mut part, &key(&sdk.root));
            }
            fresh.extend(part.records().map(|r| r.path.clone()));
            index.merge(part);
            done = true;
        }
        let fits = !done
            && file.as_ref().is_some_and(|f| {
                held || ((!crate::remote::enabled() || self.search_exe.is_none())
                    && !self.over_budget.contains(f))
            });
        if fits
            && let Some(library) = self.library(documents, uri)
            && let Some(unit) = library.uri_unit(uri)
        {
            transient = matches!(library, crate::session::Use::Own(_));
            let p = library.program();
            let lib = p.unit(unit).library;
            let paths: Vec<std::path::PathBuf> = p
                .library(lib)
                .units
                .iter()
                .filter_map(|u| p.unit(*u).path.as_deref().map(key))
                .collect();
            if again.iter().all(|a| paths.contains(a)) {
                let mut tables = crate::reference_index::Tables {
                    keys: &mut index.keys,
                    words: &mut index.words,
                    paths: &mut index.paths,
                };
                let records =
                    crate::reference_index::Builder.library(&library, lib, &root, &mut tables);
                fresh.extend(records.iter().map(|r| r.path.clone()));
                index.put(records);
                done = true;
            }
        }
        if transient && let Some(f) = file {
            self.over_budget.insert(f);
        }
        if !done {
            let sdk = self.sdk.as_ref()?;
            let built = match self.search_exe.clone() {
                Some(exe) => {
                    let request = IndexRequest {
                        sdk_lib: sdk.root.clone(),
                        uri: uri.to_string(),
                        documents: Self::document_list(documents),
                        files: Some(again.to_vec()),
                    };
                    run_index_child(&exe, &request)
                        .and_then(|b| crate::reference_index::ReferenceIndex::decode(&b))
                }
                None => crate::reference_index::build_files(sdk, documents, uri, again),
            };
            let built = built?;
            for path in again {
                if built.file(path).is_none() {
                    // Not a unit of the load (a part without its library): it is
                    // no candidate of the search either.
                    index.remove(path);
                }
            }
            fresh.extend(built.records().map(|r| r.path.clone()));
            index.merge(built);
        }
        for path in &fresh {
            if let Some(record) = index.file(path) {
                self.index_edits.remember(record, documents);
            }
        }
        Some(fresh)
    }

    /// Records again the project files whose record is not of their current
    /// text, the new ones, and dependents whose lookup or foreign ranges may change; drops
    /// the deleted ones. The document's library is recorded from the bounded
    /// session; anything else in a child (or here, without a child binary).
    fn refresh_index(
        &mut self,
        documents: &DocumentStore,
        uri: &str,
        open_files: &[std::path::PathBuf],
        whole: bool,
    ) -> bool {
        use dartforge_elements::generated::key;
        let Some(mut index) = self.reference_index.take() else {
            return false;
        };
        let had_pending = !self.index_refresh_pending.is_empty();
        let mut changes = Vec::new();
        let mut fresh = self.absorb_resident_records(documents, &mut index, &mut changes);
        let root = index.root.clone();
        // The whole project is compared with the disk after the watcher reported
        // a change, or when the last comparison is more than 30 s old (the
        // resident's rule for its scan); otherwise only the open documents.
        if whole {
            self.index_disk_dirty = false;
            self.index_disk_check = Some(std::time::Instant::now());
        }
        let open_paths: std::collections::HashSet<std::path::PathBuf> =
            open_files.iter().map(|f| key(f)).collect();
        let mut current: std::collections::HashSet<std::path::PathBuf> = if whole {
            crate::project::project_files(&root)
                .iter()
                .map(|p| key(p))
                .collect()
        } else {
            Default::default()
        };
        for f in open_files {
            if f.starts_with(&root) && f.extension().is_some_and(|e| e == "dart") {
                current.insert(key(f));
            }
        }
        let mut stale: Vec<std::path::PathBuf> = Vec::new();
        let mut declaration_changed: Vec<std::path::PathBuf> = Vec::new();
        let mut removed: Vec<std::path::PathBuf> = Vec::new();
        // Without a comparison of the whole project, only the open documents'
        // records are looked at.
        let recorded: Vec<std::path::PathBuf> = if whole {
            index.project_paths()
        } else {
            open_paths
                .iter()
                .filter(|p| index.file(p).is_some_and(|r| !r.frozen))
                .cloned()
                .collect()
        };
        for path in &recorded {
            let Some(r) = index.file(path) else { continue };
            if !whole && !open_paths.contains(path) {
                continue;
            }
            // A file of the program outside the project's walk (the generated
            // ones under `.dart_tool/build`) is gone only when it is gone from disk.
            if !current.contains(path) && !open_paths.contains(path) && !path.is_file() {
                removed.push(path.clone());
                continue;
            }
            match changed_text(r, documents) {
                None => {}
                Some(None) => removed.push(path.clone()),
                Some(Some(text)) => {
                    // Even an unchanged API can move declaration byte ranges in
                    // records of callers whose own source did not change.
                    declaration_changed.push(path.clone());
                    if !had_pending && let Some(change) = self.index_edits.change(r, &text) {
                        changes.push(change);
                    }
                    stale.push(path.clone());
                }
            }
        }
        for path in &current {
            if index.file(path).is_some() {
                continue;
            }
            let text = url::Url::from_file_path(path)
                .ok()
                .and_then(|u| documents.get(u.as_str()).map(str::to_string))
                .or_else(|| std::fs::read_to_string(path).ok());
            let stamp = text.as_deref().map(crate::reference_index::text_stamp);
            if stamp.is_some() && index.absent.get(path) == stamp.as_ref() {
                continue;
            }
            stale.push(path.clone());
        }
        declaration_changed.extend(removed.iter().cloned());
        declaration_changed.append(&mut self.index_declaration_changes);
        declaration_changed.sort();
        declaration_changed.dedup();
        let mut again: Vec<std::path::PathBuf> = stale.clone();
        let dependents = if declaration_changed.is_empty() {
            Vec::new()
        } else {
            index.dependents(&declaration_changed)
        };
        for d in dependents {
            if !again.contains(&d) && !removed.contains(&d) {
                again.push(d);
            }
        }
        for path in &self.index_refresh_pending {
            if !again.contains(path)
                && !removed.contains(path)
                && (open_paths.contains(path) || path.is_file())
            {
                again.push(path.clone());
            }
        }
        // Keep the expanded set, including importers of removed libraries,
        // until every requested record has been refreshed successfully.
        self.index_refresh_pending = again.clone();
        for path in &removed {
            index.remove(path);
        }
        if again.is_empty() {
            self.reference_index = Some(index);
            return true;
        }
        if !had_pending
            && removed.is_empty()
            && changes.len() == 1
            && declaration_changed.len() == 1
        {
            let change = changes.pop().expect("one proven source delta");
            if change.path == declaration_changed[0]
                && stale.iter().all(|path| path == &change.path)
                && change.proves_unaffected(&index, documents)
            {
                if !change.owner_is_fresh(&index, &fresh)
                    && let Some(recorded) = self.record_index_files(
                        &mut index,
                        documents,
                        uri,
                        std::slice::from_ref(&change.path),
                    )
                {
                    fresh.extend(recorded);
                }
                if change.owner_is_fresh(&index, &fresh)
                    && index.relocate_foreign_declarations(&change.path, change.edit, &fresh)
                {
                    if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
                        eprintln!(
                            "[dartforge-lsp] reference index: retained {} dependents after relocating declarations",
                            again.len()
                        );
                    }
                    self.reference_index = Some(index);
                    self.index_refresh_pending.clear();
                    return true;
                }
            }
        }
        if again
            .iter()
            .any(|path| index.file(path).is_some_and(|r| r.frozen))
        {
            // The narrow load does not include unimported package libraries.
            // Replacing only the project's records would omit their uses of
            // a declaration whose kind or signature just changed.
            let built = self.sdk.as_ref().and_then(|sdk| {
                if self.index_build.is_some() {
                    return None;
                }
                match self.search_exe.as_ref() {
                    Some(exe) => run_index_child(
                        exe,
                        &IndexRequest {
                            sdk_lib: sdk.root.clone(),
                            uri: uri.to_string(),
                            documents: Self::document_list(documents),
                            files: None,
                        },
                    )
                    .and_then(|bytes| crate::reference_index::ReferenceIndex::decode(&bytes)),
                    None => crate::reference_index::build(sdk, documents, uri),
                }
            });
            if let Some(built) = built {
                drop(index);
                self.replace_reference_index(Some(built));
                if let Some(index) = &self.reference_index {
                    for path in &declaration_changed {
                        if let Some(record) = index.file(path) {
                            self.index_edits.remember(record, documents);
                        }
                    }
                }
                return true;
            }
            self.reference_index = Some(index);
            return false;
        }
        let debug = std::env::var_os("DARTFORGE_LSP_DEBUG").is_some();
        if debug {
            for p in stale.iter().take(3) {
                eprintln!("[dartforge-lsp] reference index: stale {}", p.display());
            }
            for p in removed.iter().take(3) {
                eprintln!("[dartforge-lsp] reference index: removed {}", p.display());
            }
        }
        let started = std::time::Instant::now();
        if self
            .record_index_files(&mut index, documents, uri, &again)
            .is_none()
        {
            self.reference_index = Some(index);
            return false;
        }
        if debug {
            eprintln!(
                "[dartforge-lsp] reference index: {} files recorded again ({} with changed declarations or positions) in {} ms",
                again.len(),
                declaration_changed.len(),
                started.elapsed().as_millis()
            );
        }
        self.reference_index = Some(index);
        self.index_refresh_pending.clear();
        true
    }
}

impl SemanticAnalyzer {
    /// The implementations without the index (and the comparison's reference).
    fn unindexed_implementations(
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
}

#[cfg(test)]
mod index_refresh_tests {
    use super::*;
    use crate::typed::{RecordBatch, RecordContext, ResidentRecords};
    use dartforge_elements::sdk::SdkLayout;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct CachedIndex(PathBuf);
    impl CachedIndex {
        fn save(index: &crate::reference_index::ReferenceIndex, sdk: &SdkLayout) -> Self {
            use std::io::Write;
            let path = index_cache_path(&index.root, &sdk.root).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            let cached = Self(path);
            file.write_all(&index.encode()).unwrap();
            cached
        }
    }
    impl Drop for CachedIndex {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn fixture(label: &str) -> Directory {
        let directory = Directory(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/{label}-{}",
            std::process::id()
        )));
        let root = &directory.0;
        std::fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        std::fs::create_dir_all(root.join("project/lib")).unwrap();
        std::fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class Type {} class Function {} class num {} class int extends num {} class String {} class bool {} class Null {}",
        )
        .unwrap();
        std::fs::write(root.join("project/pubspec.yaml"), "name: example\n").unwrap();
        directory
    }

    #[test]
    fn cached_indexes_follow_project_switches_and_recheck_disk() {
        let directory = fixture("switch-project-index");
        let root = &directory.0;
        let original = "void target() {}";
        let caller = "import 'a.dart'; void run() { target(); }";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        std::fs::create_dir_all(root.join("other/lib")).unwrap();
        std::fs::write(root.join("other/pubspec.yaml"), "name: other\n").unwrap();
        std::fs::write(root.join("other/lib/a.dart"), original).unwrap();
        let a = std::fs::canonicalize(root.join("project/lib/a.dart")).unwrap();
        let b = std::fs::canonicalize(root.join("project/lib/b.dart")).unwrap();
        let other = std::fs::canonicalize(root.join("other/lib/a.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let uri_other = url::Url::from_file_path(&other).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_other.clone(), 1, original.to_string());
        let first = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        let second = crate::reference_index::build(&sdk, &documents, &uri_other).unwrap();
        let _first_cache = CachedIndex::save(&first, &sdk);
        let _second_cache = CachedIndex::save(&second, &sdk);
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.reference_index = Some(first);
        analyzer.search_exe = Some(root.join("missing-index-child"));
        let offset = original.find("target").unwrap();
        let first_answer = analyzer
            .indexed_references(&documents, &uri_a, offset)
            .unwrap()
            .unwrap();
        assert!(first_answer.uses.iter().any(|(uri, _)| uri == &uri_b));
        let other_answer = analyzer
            .indexed_references(&documents, &uri_other, offset)
            .expect("another project's ready index was permanently excluded")
            .unwrap();
        assert_eq!(
            other_answer.declaration,
            Some((uri_other.clone(), Span { start: 5, end: 11 }))
        );
        assert!(other_answer.uses.iter().all(|(uri, _)| uri != &uri_b));
        // A closed caller changes while another project's index is active.
        // Returning must scan its disk even within the previous scan's 30 s.
        let changed = format!("// shifted caller\n{caller}");
        std::fs::write(&b, &changed).unwrap();
        analyzer.search_exe = None;
        let returned = analyzer
            .indexed_references(&documents, &uri_a, offset)
            .unwrap()
            .unwrap();
        let start = changed.find("target").unwrap();
        assert!(returned.uses.contains(&(
            uri_b,
            Span {
                start,
                end: start + 6,
            },
        )));
    }

    #[test]
    fn another_projects_build_does_not_block_or_replace_cached_answers() {
        let directory = fixture("other-project-index-build");
        let root = &directory.0;
        let text = "void target() {}";
        std::fs::write(root.join("project/lib/a.dart"), text).unwrap();
        std::fs::create_dir_all(root.join("other/lib")).unwrap();
        std::fs::write(root.join("other/pubspec.yaml"), "name: other\n").unwrap();
        std::fs::write(root.join("other/lib/a.dart"), text).unwrap();
        let a = std::fs::canonicalize(root.join("project/lib/a.dart")).unwrap();
        let b = std::fs::canonicalize(root.join("other/lib/a.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, text.to_string());
        documents.open(uri_b.clone(), 1, text.to_string());
        let first = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        let second = crate::reference_index::build(&sdk, &documents, &uri_b).unwrap();
        let _first_cache = CachedIndex::save(&first, &sdk);
        let _second_cache = CachedIndex::save(&second, &sdk);
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.search_exe = Some(root.join("missing-index-child"));
        analyzer.index_build = Some((first.root.clone(), receiver));
        let offset = text.find("target").unwrap();
        for completed in [false, true] {
            if completed {
                sender.send(Some(first.encode())).unwrap();
            }
            let answer = analyzer
                .indexed_references(&documents, &uri_b, offset)
                .expect("another project's build blocked or replaced a cached answer")
                .unwrap();
            assert_eq!(
                answer.declaration,
                Some((uri_b.clone(), Span { start: 5, end: 11 }))
            );
        }
        let returned = analyzer
            .indexed_references(&documents, &uri_a, offset)
            .unwrap()
            .unwrap();
        assert_eq!(
            returned.declaration,
            Some((uri_a, Span { start: 5, end: 11 }))
        );
    }

    #[test]
    fn dependency_overlays_and_their_close_refresh_foreign_declarations() {
        let directory = fixture("dependency-overlay-index");
        let root = &directory.0;
        let original = "int target() => 0;";
        let caller = "import '../../other/lib/a.dart'; final value = target;";
        std::fs::create_dir_all(root.join("other/lib")).unwrap();
        std::fs::write(root.join("other/pubspec.yaml"), "name: other\n").unwrap();
        std::fs::write(root.join("other/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let config = root.join("other/.dart_tool/package_config.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(
            config,
            serde_json::to_vec(&serde_json::json!({
                "configVersion": 2,
                "packages": [{
                    "name": "example",
                    "rootUri": "../../project/",
                    "packageUri": "lib/",
                    "languageVersion": "3.6"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let a = dartforge_elements::config::canonicalize(&root.join("other/lib/a.dart")).unwrap();
        let b = dartforge_elements::config::canonicalize(&root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_b).unwrap();
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.replace_reference_index(Some(index));
        for (version, text, start, end) in [
            (2, "int get target => 0;", 8, 14),
            (3, "// moved\nint target() => 0;", 13, 19),
            (4, "int get target=>0;", 8, 14),
        ] {
            documents.update(&uri_a, version, text.to_string());
            let answer = analyzer
                .indexed_references(&documents, &uri_b, caller.find("target").unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(
                answer.declaration,
                Some((uri_a.clone(), Span { start, end }))
            );
        }
        let sdk = analyzer.sdk.as_ref().unwrap();
        let index = analyzer.reference_index.as_ref().unwrap();
        let project_root = index.root.clone();
        let _cache = CachedIndex::save(index, sdk);
        assert!(load_cached_index(&project_root, &sdk.root, &documents, &b).is_some());
        documents.close(&uri_a);
        assert!(load_cached_index(&project_root, &sdk.root, &documents, &b).is_none());
        let answer = analyzer
            .indexed_references(&documents, &uri_b, caller.find("target").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            answer.declaration,
            Some((uri_a, Span { start: 4, end: 10 }))
        );
        // From the dependency's own root, the caller is a frozen package.
        // Changing the declaration kind must keep that external use too.
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let index =
            crate::reference_index::build(analyzer.sdk.as_ref().unwrap(), &documents, &uri_a)
                .unwrap();
        assert!(index.file(&b).is_some_and(|r| r.frozen));
        analyzer.replace_reference_index(Some(index));
        documents.open(uri_a.clone(), 6, "int get target=>0;".to_string());
        let answer = analyzer
            .indexed_references(&documents, &uri_a, 8)
            .unwrap()
            .unwrap();
        assert!(
            answer.uses.iter().any(|(uri, _)| uri == &uri_b),
            "an API refresh omitted the dependent in another package"
        );
    }

    #[test]
    fn closed_dependency_disk_changes_refresh_foreign_declarations() {
        let directory = fixture("closed-dependency-index");
        let root = &directory.0;
        let original = "int target() => 0;";
        let caller = "import '../../other/lib/a.dart'; final value = target;";
        std::fs::create_dir_all(root.join("other/lib")).unwrap();
        std::fs::write(root.join("other/pubspec.yaml"), "name: other\n").unwrap();
        std::fs::write(root.join("other/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = std::fs::canonicalize(root.join("other/lib/a.dart")).unwrap();
        let b = std::fs::canonicalize(root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_b).unwrap();
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.replace_reference_index(Some(index));
        let offset = caller.find("target").unwrap();
        analyzer
            .indexed_references(&documents, &uri_b, offset)
            .unwrap();
        for (watcher, text, start, end) in [
            (true, "int get target => 0;", 8, 14),
            (false, "// moved\nint target() => 0;", 13, 19),
        ] {
            std::fs::write(&a, text).unwrap();
            if watcher {
                analyzer.disk_changed(std::slice::from_ref(&a));
            } else {
                analyzer.index_disk_check =
                    Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
            }
            let answer = analyzer
                .indexed_references(&documents, &uri_b, offset)
                .unwrap()
                .unwrap();
            assert_eq!(
                answer.declaration,
                Some((uri_a.clone(), Span { start, end })),
                "the closed dependency's disk text was not checked"
            );
        }
        // An identical rewrite must retain the complete index even when a
        // watcher reports it; rebuilding through this child would fail.
        std::fs::write(&a, "// moved\nint target() => 0;").unwrap();
        analyzer.disk_changed(std::slice::from_ref(&a));
        analyzer.search_exe = Some(root.join("missing-index-child"));
        assert!(
            analyzer
                .indexed_references(&documents, &uri_b, offset)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn cached_indexes_follow_package_configuration_changes() {
        let directory = fixture("configured-reference-index");
        let root = &directory.0;
        for (name, text) in [
            ("old", "int target() => 0;"),
            ("new", "int get target => 0;"),
        ] {
            std::fs::create_dir_all(root.join(name).join("lib")).unwrap();
            std::fs::write(root.join(name).join("lib/dep.dart"), text).unwrap();
        }
        let caller = "import 'package:dep/dep.dart'; final value = target;";
        std::fs::write(root.join("project/lib/main.dart"), caller).unwrap();
        let config = root.join("project/.dart_tool/package_config.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        let configure = |name: &str| {
            std::fs::write(
                &config,
                serde_json::to_vec(&serde_json::json!({
                    "configVersion": 2,
                    "packages": [{
                        "name": "dep",
                        "rootUri": format!("../../{name}/"),
                        "packageUri": "lib/",
                        "languageVersion": "3.6"
                    }]
                }))
                .unwrap(),
            )
            .unwrap();
        };
        configure("old");
        let configuration_metadata = std::fs::metadata(&config).unwrap();
        let configuration_time = configuration_metadata.modified().unwrap();
        let configuration_stamp =
            dartforge_elements::unit_cache::FileStamp::of(&configuration_metadata);
        // A config restored by another tool can keep its old date and size.
        let preserve_metadata = || {
            std::fs::File::options()
                .write(true)
                .open(&config)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(configuration_time))
                .unwrap();
            assert_eq!(
                dartforge_elements::unit_cache::FileStamp::of(&std::fs::metadata(&config).unwrap()),
                configuration_stamp
            );
        };
        let main = std::fs::canonicalize(root.join("project/lib/main.dart")).unwrap();
        let old = std::fs::canonicalize(root.join("old/lib/dep.dart")).unwrap();
        let new = std::fs::canonicalize(root.join("new/lib/dep.dart")).unwrap();
        let uri = url::Url::from_file_path(&main).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri).unwrap();
        let _cache = CachedIndex::save(&index, &sdk);
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        let offset = caller.find("target").unwrap();
        let before = analyzer
            .semantic_references(&documents, &uri, offset)
            .unwrap();
        assert_eq!(
            before.declaration,
            Some((
                url::Url::from_file_path(&old).unwrap().to_string(),
                Span { start: 4, end: 10 }
            ))
        );
        configure("old");
        analyzer.project_configuration_changed();
        analyzer.search_exe = Some(root.join("missing-index-child"));
        assert!(
            analyzer
                .indexed_references(&documents, &uri, offset)
                .expect("an identical configuration rewrite discarded the cache")
                .is_some()
        );
        analyzer.search_exe = None;
        configure("new");
        preserve_metadata();
        analyzer.project_configuration_changed();
        let changed = analyzer
            .semantic_references(&documents, &uri, offset)
            .unwrap();
        assert_eq!(
            changed.declaration,
            Some((
                url::Url::from_file_path(&new).unwrap().to_string(),
                Span { start: 8, end: 14 }
            )),
            "the cache reused the previous package map"
        );
        let index = crate::reference_index::build(analyzer.sdk.as_ref().unwrap(), &documents, &uri)
            .unwrap();
        analyzer.replace_reference_index(Some(index));
        configure("old");
        preserve_metadata();
        analyzer.index_disk_check =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
        let restored = analyzer
            .semantic_references(&documents, &uri, offset)
            .unwrap();
        assert_eq!(restored.declaration, before.declaration);
        let nested = root.join("project/lib/.dart_tool/package_config.json");
        std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
        std::fs::write(
            &nested,
            serde_json::to_vec(&serde_json::json!({
                "configVersion": 2,
                "packages": [{
                    "name": "dep",
                    "rootUri": "../../../new/",
                    "packageUri": "lib/",
                    "languageVersion": "3.6"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        analyzer.project_configuration_changed();
        let overridden = analyzer
            .semantic_references(&documents, &uri, offset)
            .unwrap();
        assert_eq!(overridden.declaration, changed.declaration);
        std::fs::remove_file(nested).unwrap();
        analyzer.project_configuration_changed();
        let inherited = analyzer
            .semantic_references(&documents, &uri, offset)
            .unwrap();
        assert_eq!(inherited.declaration, before.declaration);
    }

    #[test]
    fn a_failed_refresh_keeps_dependents_pending() {
        let directory = fixture("pending-index-refresh");
        let root = &directory.0;
        let original = "void target() {}";
        let caller = "import 'a.dart'; void run() { target(); }";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = std::fs::canonicalize(root.join("project/lib/a.dart")).unwrap();
        let b = std::fs::canonicalize(root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        documents.update(&uri_a, 2, "void renamed() {}".to_string());
        let edited =
            crate::reference_index::build_files(&sdk, &documents, &uri_a, std::slice::from_ref(&a))
                .unwrap();
        let records = Arc::new(ResidentRecords::default());
        records.lock().ready.push(RecordBatch {
            bytes: edited.encode(),
            context: RecordContext::current(&sdk.root, &documents).unwrap(),
        });
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.reference_index = Some(index);
        analyzer.resident_records = Some(records);
        analyzer.over_budget.insert(b);
        analyzer.search_exe = Some(root.join("missing-index-child"));
        let offset = caller.find("target").unwrap();
        // The resident supplies the edited file, but rebuilding its importer
        // fails. Both requests must fall back to the current, unresolved call.
        assert!(
            analyzer
                .semantic_references(&documents, &uri_b, offset)
                .is_none()
        );
        assert!(
            analyzer
                .semantic_references(&documents, &uri_b, offset)
                .is_none(),
            "a failed refresh made stale dependent records queryable"
        );
        analyzer.search_exe = None;
        let repaired = caller.replace("target", "renamed");
        documents.update(&uri_b, 2, repaired.clone());
        let references = analyzer
            .semantic_references(&documents, &uri_b, repaired.find("renamed").unwrap())
            .unwrap();
        assert_eq!(
            references.declaration,
            Some((uri_a, Span { start: 5, end: 12 }))
        );
        assert!(references.uses.iter().any(|(uri, _)| uri == &uri_b));
    }

    #[test]
    fn a_superseded_context_does_not_replace_current_references() {
        let directory = fixture("superseded-record-context");
        let root = &directory.0;
        let original = "int target() => 0;";
        let caller = "import 'a.dart'; final value = target;";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = std::fs::canonicalize(root.join("project/lib/a.dart")).unwrap();
        let b = std::fs::canonicalize(root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        documents.update(&uri_a, 2, "int get target => 0;".to_string());
        let stale =
            crate::reference_index::build_files(&sdk, &documents, &uri_b, std::slice::from_ref(&b))
                .unwrap();
        // The caller's text is identical, but this batch resolved it while
        // another open document exposed a different API.
        let context = RecordContext::current(&sdk.root, &documents).unwrap();
        documents.update(&uri_a, 3, original.to_string());
        let records = Arc::new(ResidentRecords::default());
        records.lock().ready.push(RecordBatch {
            bytes: stale.encode(),
            context,
        });
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.reference_index = Some(index);
        analyzer.resident_records = Some(records);
        let references = analyzer
            .semantic_references(&documents, &uri_b, caller.find("target").unwrap())
            .unwrap();
        assert_eq!(
            references.declaration,
            Some((uri_a, Span { start: 4, end: 10 })),
            "a caller's unchanged text hid the stale context of its records"
        );
    }

    #[test]
    fn a_restored_context_keeps_the_complete_index_usable() {
        let directory = fixture("restored-record-context");
        let root = &directory.0;
        let original = "void target() {}";
        let caller = "import 'a.dart'; void run() { target(); }";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = std::fs::canonicalize(root.join("project/lib/a.dart")).unwrap();
        let b = std::fs::canonicalize(root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        let records = Arc::new(ResidentRecords::default());
        for (version, text) in [(2, "void renamed() {}"), (3, original)] {
            documents.update(&uri_a, version, text.to_string());
            let part = crate::reference_index::build_files(
                &sdk,
                &documents,
                &uri_a,
                std::slice::from_ref(&a),
            )
            .unwrap();
            records.lock().ready.push(RecordBatch {
                bytes: part.encode(),
                context: RecordContext::current(&sdk.root, &documents).unwrap(),
            });
        }
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.reference_index = Some(index);
        analyzer.resident_records = Some(records);
        analyzer.over_budget.insert(b);
        analyzer.search_exe = Some(root.join("missing-index-child"));
        let references = analyzer
            .indexed_references(&documents, &uri_b, caller.find("target").unwrap())
            .expect("a superseded batch unnecessarily invalidated a complete current index")
            .unwrap();
        assert_eq!(
            references.declaration,
            Some((uri_a, Span { start: 5, end: 11 }))
        );
        assert!(references.uses.iter().any(|(uri, _)| uri == &uri_b));
    }
    #[test]
    fn unchanged_api_edits_relocate_foreign_declarations() {
        let directory = fixture("unchanged-api-relocation");
        let root = &directory.0;
        let original = "int helper() { return 0; }\r\nint target() => 1;\r\n";
        let caller = "import 'a.dart'; final value = target;";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = dartforge_elements::config::canonicalize(&root.join("project/lib/a.dart")).unwrap();
        let b = dartforge_elements::config::canonicalize(&root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        let records = Arc::new(ResidentRecords::default());
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.resident_records = Some(records.clone());
        analyzer.replace_reference_index(Some(index));
        analyzer.search_exe = Some(root.join("missing-index-child"));
        analyzer.over_budget.insert(b.clone());
        let comment = original.replace("int target", "// 🧭 moved\r\nint target");
        let body = original.replace("return 0;", "final value = 0;\r\n  return value;");
        for (version, text) in [(2, comment), (3, body), (4, original.to_string())] {
            assert_eq!(
                dartforge_elements::incremental::api_signature(&text),
                dartforge_elements::incremental::api_signature(original)
            );
            documents.update(&uri_a, version, text.clone());
            let part = crate::reference_index::build_files(
                analyzer.sdk.as_ref().unwrap(),
                &documents,
                &uri_a,
                std::slice::from_ref(&a),
            )
            .unwrap();
            records.lock().ready.push(RecordBatch {
                bytes: part.encode(),
                context: RecordContext::current(&analyzer.sdk.as_ref().unwrap().root, &documents)
                    .unwrap(),
            });
            let answer = analyzer
                .indexed_references(&documents, &uri_b, caller.find("target").unwrap())
                .expect("a body/comment edit forced the unavailable broad index child")
                .unwrap();
            let start = text.find("target").unwrap();
            assert_eq!(
                answer.declaration,
                Some((
                    uri_a.clone(),
                    Span {
                        start,
                        end: start + 6
                    }
                ))
            );
            let line = text[..start].bytes().filter(|c| *c == b'\n').count() as u32;
            assert_eq!(answer.declaration_range, Some((line, 4, line, 10)));
        }
    }

    #[test]
    fn isolated_function_edits_relocate_foreign_ranges_across_unsaved_changes() {
        let directory = fixture("selective-api-relocation");
        let root = &directory.0;
        let original = "// 🧭\r\nint target() => 0;\r\n";
        let caller = "import 'a.dart'; final value = target;";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = dartforge_elements::config::canonicalize(&root.join("project/lib/a.dart")).unwrap();
        let b = dartforge_elements::config::canonicalize(&root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        let records = Arc::new(ResidentRecords::default());
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.resident_records = Some(records.clone());
        analyzer.replace_reference_index(Some(index));
        assert_eq!(
            records.lock().wanted.as_ref(),
            analyzer.reference_index.as_ref().map(|index| &index.root)
        );
        analyzer.search_exe = Some(root.join("missing-index-child"));
        analyzer.over_budget.insert(b.clone());
        let first = original.replace("int target", "int unused() => 1;\r\nint target");
        let second = first.replace("int target", "String another() => '';\r\nint target");
        let third = original.replace("int target", "String another() => '';\r\nint target");
        for (version, text) in [
            (2, first),
            (3, second),
            (4, third),
            (5, original.to_string()),
        ] {
            documents.update(&uri_a, version, text.clone());
            // A resident can refresh an unchanged caller too. Its current
            // foreign ranges must not be translated a second time.
            let files = if version == 3 {
                vec![a.clone(), b.clone()]
            } else {
                vec![a.clone()]
            };
            let part = crate::reference_index::build_files(
                analyzer.sdk.as_ref().unwrap(),
                &documents,
                &uri_a,
                &files,
            )
            .unwrap();
            records.lock().ready.push(RecordBatch {
                bytes: part.encode(),
                context: RecordContext::current(&analyzer.sdk.as_ref().unwrap().root, &documents)
                    .unwrap(),
            });
            let answer = analyzer
                .indexed_references(&documents, &uri_b, caller.find("target").unwrap())
                .expect("an isolated unused function forced the unavailable broad index child")
                .unwrap();
            let start = text.find("target").unwrap();
            assert_eq!(
                answer.declaration,
                Some((
                    uri_a.clone(),
                    Span {
                        start,
                        end: start + 6
                    }
                ))
            );
            let line = text[..start].bytes().filter(|c| *c == b'\n').count() as u32;
            assert_eq!(answer.declaration_range, Some((line, 4, line, 10)));
            assert!(answer.uses.iter().any(|(uri, _)| uri == &uri_b));
        }
    }

    #[test]
    fn an_isolated_function_edit_refreshes_only_its_owner_before_relocating_callers() {
        let directory = fixture("selective-owner-refresh");
        let root = &directory.0;
        let original = "int target() => 0;";
        let caller = "import 'a.dart'; final value = target;";
        std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
        std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
        let a = dartforge_elements::config::canonicalize(&root.join("project/lib/a.dart")).unwrap();
        let b = dartforge_elements::config::canonicalize(&root.join("project/lib/b.dart")).unwrap();
        let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
        let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
        let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
        let mut documents = DocumentStore::new();
        documents.open(uri_a.clone(), 1, original.to_string());
        documents.open(uri_b.clone(), 1, caller.to_string());
        let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        analyzer.replace_reference_index(Some(index));
        drop(analyzer.library(&documents, &uri_a).unwrap());
        analyzer.search_exe = Some(root.join("missing-index-child"));
        let text = format!("int unused() => 1;\n{original}");
        documents.update(&uri_a, 2, text.clone());
        let answer = analyzer
            .indexed_references(&documents, &uri_a, text.find("target").unwrap())
            .expect("the owner's fresh library did not avoid a broad child rebuild")
            .unwrap();
        assert!(answer.uses.iter().any(|(uri, _)| uri == &uri_b));
        let from_caller = analyzer
            .indexed_references(&documents, &uri_b, caller.find("target").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(from_caller.declaration, answer.declaration);
    }

    #[test]
    fn unresolved_and_documented_names_do_not_enter_selective_api_refresh() {
        for (label, caller, part) in [
            ("unresolved", "import 'a.dart'; final value = added;", false),
            (
                "documentation",
                "import 'a.dart'; /// Refers to [added].\nvoid run() {}",
                false,
            ),
            ("part", "import 'a.dart'; final value = target;", true),
        ] {
            let directory = fixture(&format!("selective-name-{label}"));
            let root = &directory.0;
            let original = if part {
                "part 'c.dart'; int target() => 0;"
            } else {
                "int target() => 0;"
            };
            if part {
                std::fs::write(
                    root.join("project/lib/c.dart"),
                    "part of 'a.dart'; final result = added;",
                )
                .unwrap();
            }
            std::fs::write(root.join("project/lib/a.dart"), original).unwrap();
            std::fs::write(root.join("project/lib/b.dart"), caller).unwrap();
            let a =
                dartforge_elements::config::canonicalize(&root.join("project/lib/a.dart")).unwrap();
            let b =
                dartforge_elements::config::canonicalize(&root.join("project/lib/b.dart")).unwrap();
            let uri_a = url::Url::from_file_path(&a).unwrap().to_string();
            let uri_b = url::Url::from_file_path(&b).unwrap().to_string();
            let sdk = SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap();
            let mut documents = DocumentStore::new();
            documents.open(uri_a.clone(), 1, original.to_string());
            documents.open(uri_b.clone(), 1, caller.to_string());
            let index = crate::reference_index::build(&sdk, &documents, &uri_a).unwrap();
            let mut analyzer = SemanticAnalyzer::new(Some(sdk));
            analyzer.replace_reference_index(Some(index));
            drop(analyzer.library(&documents, &uri_a).unwrap());
            analyzer.search_exe = Some(root.join("missing-index-child"));
            let text = original.replace("int target", "int added() => 1;\nint target");
            documents.update(&uri_a, 2, text.clone());
            assert!(
                matches!(
                    analyzer.indexed_references(&documents, &uri_a, text.find("target").unwrap()),
                    Err(crate::reference_index::Miss::Unsupported(_))
                ),
                "a {label} name was ignored when retaining its dependent"
            );
            analyzer.search_exe = None;
            let complete = analyzer
                .indexed_references(&documents, &uri_a, text.find("added").unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(
                complete.declaration,
                Some((
                    uri_a.clone(),
                    Span {
                        start: text.find("added").unwrap(),
                        end: text.find("added").unwrap() + 5
                    }
                ))
            );
            assert!(analyzer.index_refresh_pending.is_empty());
            if label == "unresolved" {
                assert!(complete.uses.iter().any(|(uri, _)| uri == &uri_b));
            }
        }
    }

    #[test]
    fn opening_a_record_without_identifier_facts_uses_current_library_fallback() {
        let directory = fixture("index-open-without-facts");
        let base = &directory.0;
        let api = "class Item { int field = 1; void work() {} } Item makeItem() => Item();";
        let caller = "import 'api.dart'; void call() { makeItem().work(); }";
        let closed = "// 🧭\r\nimport 'api.dart';\r\nvoid closed() {\r\n final value = makeItem();\r\n value.work();\r\n print(value.field);\r\n}\r\n";
        let root = base.join("project/lib");
        for (name, text) in [
            ("api.dart", api),
            ("caller.dart", caller),
            ("closed.dart", closed),
        ] {
            std::fs::write(root.join(name), text).unwrap();
        }
        let uri_api = url::Url::from_file_path(root.join("api.dart"))
            .unwrap()
            .to_string();
        let path = dartforge_elements::generated::key(
            &std::fs::canonicalize(root.join("closed.dart")).unwrap(),
        );
        let uri = url::Url::from_file_path(&path).unwrap().to_string();
        let mut documents = DocumentStore::new();
        documents.open(uri_api.clone(), 1, api.to_owned());
        let sdk = SdkLayout::load(&base.join("sdk/lib"), "dartdevc").unwrap();
        let mut index = crate::reference_index::build(&sdk, &documents, &uri_api).unwrap();
        let before = index.encode();
        let mut record = index.file(&path).unwrap().clone();
        assert!(!record.facts.is_empty());
        record.idents.clear();
        record.facts.clear();
        record.prepares.clear();
        record.prepare_facts.clear();
        index.put(vec![record]);
        let mut cached = SemanticAnalyzer::new(Some(
            SdkLayout::load(&base.join("sdk/lib"), "dartdevc").unwrap(),
        ));
        cached.replace_reference_index(crate::reference_index::ReferenceIndex::decode(&before));
        let mut deferred = SemanticAnalyzer::new(Some(sdk));
        deferred.replace_reference_index(crate::reference_index::ReferenceIndex::decode(
            &index.encode(),
        ));
        documents.open(uri.clone(), 1, closed.to_owned());
        for name in ["makeItem", "work", "field", "value"] {
            let offset = closed.rfind(name).unwrap() + 1;
            let expected = cached.references_in(&documents, &uri, offset).unwrap();
            let actual = deferred.references_in(&documents, &uri, offset).unwrap();
            assert_eq!(actual.declaration, expected.declaration, "{name}");
            assert_eq!(actual.uses, expected.uses, "{name}");
            assert_eq!(
                deferred.prepare_rename(&documents, &uri, offset),
                cached.prepare_rename(&documents, &uri, offset),
                "rename {name}"
            );
        }
        assert!(
            deferred.index_answers.1 > 0,
            "library fallback was not exercised"
        );
        assert!(
            deferred
                .reference_index
                .as_ref()
                .unwrap()
                .file(&path)
                .unwrap()
                .facts
                .is_empty(),
            "test unexpectedly rebuilt the optional caches"
        );
    }
}
