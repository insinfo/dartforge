//! The LSP semantic analyzer. The program, the trees and the type table of a
//! query live, at most, in the bounded session of [`crate::session`] (one
//! entry, discarded at each text change); completion, which analyzes a text
//! with a sentinel, is always transient.

use crate::{Analyzer, SyntacticAnalyzer, DocumentStore, Hover, References};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::{generated::GenerationBuilder, load::load_lenient_generated, model::{Program, UnitId}, sdk::SdkLayout};
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
        Self { syntactic: SyntacticAnalyzer::new(), sdk, session: crate::session::Session::new(), wide_session: crate::session::Session::wide(), known: Default::default(), completion_max: crate::completion::MAX_PATTERN, not_imported_in_completion: true, incremental: std::env::var("DARTFORGE_LSP_INCREMENTAL").map(|v| v.trim() != "0").unwrap_or(true) }
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
    pub(crate) fn library(&mut self, documents: &DocumentStore, uri: &str) -> Option<crate::session::Use<'_>> {
        let sdk = self.sdk.as_ref()?;
        let file = crate::project::uri_file(uri)?;
        self.session.get(crate::session::Scope::Library(file), documents, || {
            crate::project::load_library(sdk, documents, uri)
        })
    }

    /// The whole project of `uri` (references, rename), from the session or loaded.
    pub(crate) fn project(&mut self, documents: &DocumentStore, uri: &str) -> Option<crate::session::Use<'_>> {
        let sdk = self.sdk.as_ref()?;
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        self.session.get(crate::session::Scope::Project(root), documents, || {
            crate::project::load_project(sdk, documents, uri)
        })
    }

    /// The wide load of `uri` with the bodies of the libraries that cite or
    /// declare `name` inferred (the candidates of `_addResults`, §11.2), from
    /// the wide session or loaded.
    pub(crate) fn wide(&mut self, documents: &DocumentStore, uri: &str, name: &str) -> Option<crate::session::Use<'_>> {
        let SemanticAnalyzer { wide_session, sdk: Some(sdk), .. } = self else { return None };
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        let mut usage = wide_session.get(crate::session::Scope::Project(root), documents, || {
            crate::project::load_wide_project(sdk, documents, uri)
        })?;
        usage.widen_to(name);
        Some(usage)
    }

    /// Discovers the SDK through the same paths the compiler uses.
    pub fn discover() -> Self {
        // The SDK as the analyzer sees it (M1): without patches, with the dart2js
        // internal libraries.
        let sdk = SdkLayout::discover().and_then(|p| SdkLayout::load_as_analyzer(&p).ok());
        Self::new(sdk)
    }

    /// In-memory generation with the current texts of the open `.dart`
    /// documents: during the load, they win over the disk.
    pub(crate) fn open(documents: &DocumentStore) -> GenerationBuilder {
        let mut generator = GenerationBuilder::new();
        for open in documents.uris() {
            let Some(source) = documents.get(open) else { continue };
            // The canonical path (the disk's case on a case-insensitive file system),
            // the same as `uri_file`'s.
            let Some(file) = crate::project::uri_file(open) else { continue };
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
    fn define(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<(String, Option<Span>)> {
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
        self.syntactic.definition_in_workspace(uri, text, offset, documents)
    }

    /// Hover through semantic identity; without it, the conservative syntactic one.
    fn pass_hover(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<Hover> {
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
        self.syntactic.hover_in_workspace(uri, text, offset, documents)
    }

    /// References through declaration identity, in every library of the project
    /// (open or only on disk). Declarations from dependencies (SDK, packages)
    /// come in as declarations, but uses are only searched for in the project.
    fn semantic_references(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        // Non-local search: the wide load, with the SDK and the packages that cite
        // the name (docs/LSP-ESPECIFICACAO.md §11.9 B); a local one stays in the
        // project.
        let name = {
            let project = self.project(documents, uri)?;
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)
        };
        if let Some(name) = name
            && let Some(wide) = self.wide(documents, uri, &name)
        {
            return references_in(&wide, uri, offset);
        }
        let project = self.project(documents, uri)?;
        references_in(&project, uri, offset)
    }
}

/// The references of what is at `offset` of `uri`, in the given project.
fn references_in(project: &crate::project::Project, uri: &str, offset: usize) -> Option<References> {
    {
        let unit = project.uri_unit(uri)?;
        let d = project.identify(unit, offset).ok()??;
        if let crate::project::Target::Prefix { library, name } = d.target {
            let (declaration, uses) = project.prefix_references(unit, offset, library, name)?;
            let mut uses: Vec<(String, Span)> = uses.into_iter().filter_map(|(u, s)| Some((project.unit_uri(u)?, s))).collect();
            uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
            let declaration = declaration.and_then(|(u, s)| Some((project.unit_uri(u)?, s)));
            return Some(References { declaration, uses });
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
        for (u, from, until) in project.occurrences_in(&d.target, false, candidates.as_ref()).ok()? {
            if declarations.contains(&(u, from, until)) {
                continue;
            }
            if candidates.as_ref().is_some_and(|c| !c.contains(&u)) {
                continue;
            }
            uses.push((project.unit_uri(u)?, Span { start: from, end: until }));
        }
        uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        let declaration = declaration.and_then(|(u, s)| Some((project.unit_uri(u)?, s)));
        Some(References { declaration, uses })
    }
}

/// Loads the program of `uri` with `text` and the other open documents with
/// their current texts (the other files come from disk). A part comes in
/// through its owning library.
pub(crate) fn load(sdk: &SdkLayout, uri: &str, text: &str, documents: Option<&DocumentStore>) -> Option<(Program, Interner, UnitId)> {
    let path = crate::project::uri_file(uri)?;
    if path.extension().is_none_or(|e| e != "dart") { return None; }
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
    for entry in owning.iter().chain([&path]) {
        let mut names = Interner::new();
        let (program, _) = load_lenient_generated(entry, sdk, None, &mut names, None, None, Some(generation.clone()));
        let unit = program.units.iter().position(|u| u.path.as_deref().map(dartforge_elements::generated::key).as_ref() == Some(&key));
        if let Some(unit) = unit {
            return Some((program, names, UnitId(unit as u32)));
        }
    }
    None
}

/// The owning library of `path` when the text is a part (`part of`): through
/// the written URI (relative or `package:`), or, in the old `part of name;`
/// form, through the project file that declares `part` for it.
pub(crate) fn owning_library(path: &std::path::Path, text: &str, documents: Option<&DocumentStore>) -> Option<std::path::PathBuf> {
    use dartforge_elements::generated::key;
    use dartforge_frontend::ast::DirectiveKind;
    if !text.contains("part") {
        return None;
    }
    let mut names = Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    let uri = analyzed.unit.directives.iter().find_map(|d| match &d.kind {
        DirectiveKind::PartOf { uri, .. } => Some(uri.as_ref().and_then(dartforge_elements::load::string_lit_value)),
        _ => None,
    })?;
    if let Some(uri) = uri {
        if uri.starts_with("package:") {
            let config = dartforge_elements::config::PackageConfig::discover(path)?;
            return dartforge_elements::config::PackageConfig::load(&config).ok()?.resolve_package_uri(&uri).ok();
        }
        return Some(key(&path.parent()?.join(uri)));
    }
    let target = key(path);
    let root = crate::project::project_root(path);
    crate::project::project_files(&root).into_iter().find(|candidate| {
        let open = documents
            .and_then(|d| Url::from_file_path(candidate).ok().and_then(|u| d.get(u.as_str()).map(str::to_string)));
        let Some(source) = open.or_else(|| std::fs::read_to_string(candidate).ok()) else { return false };
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

    fn definition(&mut self, uri: &str, text: &str, offset: usize) -> Option<(String, Option<Span>)> {
        let only_this = Self::only_this(uri, text);
        self.define(uri, text, offset, &only_this)
    }

    fn definition_in_workspace(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<(String, Option<Span>)> {
        self.define(uri, text, offset, documents)
    }

    fn references(&mut self, uri: &str, text: &str, offset: usize) -> Option<Vec<Span>> {
        self.syntactic.references(uri, text, offset)
    }

    fn references_in(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        if self.sdk.is_some() && documents.get(uri).is_some() {
            return self.semantic_references(documents, uri, offset);
        }
        self.syntactic.references_in(documents, uri, offset)
    }

    fn hover(&mut self, uri: &str, text: &str, offset: usize) -> Option<Hover> {
        let only_this = Self::only_this(uri, text);
        self.pass_hover(uri, text, offset, &only_this)
    }

    fn hover_in_workspace(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<Hover> {
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

    fn prepare_rename(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Result<Option<(Span, String)>, String> {
        let Some(project) = self.project(documents, uri) else { return Ok(None) };
        crate::rename::prepare(&project, uri, offset)
    }

    fn rename(&mut self, documents: &DocumentStore, uri: &str, offset: usize, new: &str) -> Result<crate::Rename, String> {
        let project = self.project(documents, uri)
            .ok_or_else(|| "The file's project could not be loaded (missing SDK or a URI that is not a file).".to_string())?;
        crate::rename::rename(&project, uri, offset, new)
    }

    fn actions(&mut self, documents: &DocumentStore, uri: &str, start: usize, end: usize, published_: &[Diagnostic]) -> Vec<crate::CodeAction> {
        let Some(text) = documents.get(uri) else { return Vec::new() };
        // The immediate ones of the current version and the typed ones already
        // published for it.
        let mut diagnostics = self.diagnose(uri, text);
        for d in published_ {
            if !diagnostics.iter().any(|x| x.code == d.code && x.span == d.span) {
                diagnostics.push(d.clone());
            }
        }
        // The fixes apply to the diagnostics of the requested lines (the Dart
        // server filters by line); the assists, to the cursor.
        let (line_start_, line_end) = {
            let a = text[..start.min(text.len())].rfind('\n').map_or(0, |i| i + 1);
            let b = text[end.min(text.len())..].find('\n').map_or(text.len(), |i| end + i);
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
        let ignore = crate::ignore_diagnostic::ignore(uri, text, &diagnostics, line_start_, line_end);
        if self.sdk.is_none() {
            output.extend(ignore);
            return output;
        }
        let Some(file) = crate::project::uri_file(uri) else {
            output.extend(ignore);
            return output;
        };
        let SemanticAnalyzer { session, sdk: Some(sdk), known, .. } = self else {
            output.extend(ignore);
            return output;
        };
        if let Some(mut project) = session.get(crate::session::Scope::Library(file), documents, || {
            crate::project::load_library(sdk, documents, uri)
        }) {
            let published = crate::actions::fix_published(&project, uri, &diagnostics, line_start_, line_end);
            let all = crate::actions::fix_in_whole_file(&published, &diagnostics, text, |o, a, b| {
                crate::actions::fix_published(&project, uri, std::slice::from_ref(o), a, b)
            });
            output.extend(published);
            output.extend(all);
            // Import before create: the priority of Dart's fixes.
            output.extend(crate::actions::import(&project, Some(&*sdk), known, documents, uri, line_start_, line_end));
            let fixed = crate::fixes::fix(&mut project, uri, &diagnostics, line_start_, line_end);
            let all = crate::actions::fix_in_whole_file(&fixed, &diagnostics, text, |o, a, b| {
                crate::fixes::fix(&mut project, uri, std::slice::from_ref(o), a, b)
            });
            output.extend(fixed);
            output.extend(all);
            output.extend(crate::fixes::required_arguments(&project, uri, line_start_, line_end));
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
                    if let Some((span, text)) = cx.convert_to_block_body(dartforge_diagnostics::Span { start, end }) {
                        output.push(crate::fixes::block_body(crate::actions::CodeAction {
                            title: "Convert to block body".into(),
                            kind: "refactor.convert.bodyToBlock".into(),
                            edits: vec![crate::Edit { uri: uri.to_string(), span, text }],
                            diagnostic: None,
                            create_file: None, linked: Default::default(),
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

    fn refactorings(&mut self, documents: &DocumentStore, uri: &str, offset: usize, length: usize, create_files: bool) -> Vec<crate::Refactoring> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.refactorings(unit, offset, length, create_files)
    }

    fn run_refactoring(&mut self, documents: &DocumentStore, uri: &str, request: &crate::RefactoringRequest) -> crate::RefactoringResult {
        let Some(project) = self.project(documents, uri) else { return crate::RefactoringResult::NotAnalyzed };
        crate::refactorings_exec::run(&project, uri, request)
    }

    fn move_to_file(&mut self, documents: &DocumentStore, uri: &str, offset: usize, length: usize, target: &str) -> crate::RefactoringResult {
        let Some(project) = self.project(documents, uri) else { return crate::RefactoringResult::NotAnalyzed };
        crate::refactorings_exec::move_(&project, uri, offset, length, target)
    }

    fn folds(&mut self, uri: &str, text: &str, lines_only: bool) -> Vec<crate::Fold> {
        self.syntactic.folds(uri, text, lines_only)
    }

    fn selections(&mut self, uri: &str, text: &str, offset: usize) -> Vec<Span> {
        self.syntactic.selections(uri, text, offset)
    }

    fn signature(&mut self, documents: &DocumentStore, uri: &str, offset: usize, auto: bool) -> Option<crate::Signature> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.signature(unit, offset, auto)
    }

    fn document_highlights(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<Vec<Span>> {
        if self.sdk.is_none() {
            let text = documents.get(uri)?.to_string();
            return self.syntactic.references(uri, &text, offset);
        }
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.document_highlights(unit, offset)
    }

    fn implementations(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Vec<(String, Span)> {
        // `searchAllSubtypes` searches every known file, including the whole SDK
        // (`JSString` from `dart:_interceptors` implements `String`): the wide
        // load, like the references'.
        let name = self.project(documents, uri).and_then(|project| {
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)
        });
        if let Some(name) = name
            && let Some(wide) = self.wide(documents, uri, &name)
            && let Some(unit) = wide.uri_unit(uri)
        {
            return wide.implementations(unit, offset).into_iter().filter_map(|(u, s)| Some((wide.unit_uri(u)?, s))).collect();
        }
        let Some(project) = self.project(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
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
            let Some(project) = self.library(documents, uri) else { return Vec::new() };
            let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
            project.color_libraries(unit)
        };
        if extras.is_empty() {
            let Some(mut project) = self.library(documents, uri) else { return Vec::new() };
            let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
            return project.colors(unit);
        }
        let Some(sdk) = self.sdk.as_ref() else { return Vec::new() };
        let Some(project) = crate::project::load_library(sdk, documents, uri) else { return Vec::new() };
        let crate::project::Project { query, root, mut libraries, referenced_names, reverse_index, comments, occurrences_by_unit, written_subtypes } = project;
        let mut list: Vec<dartforge_elements::model::LibraryId> = libraries.iter().copied().chain(extras.iter().copied()).collect();
        list.sort();
        list.dedup();
        libraries.extend(extras);
        let mut project = crate::project::Project { query: query.reinfer(&list), root, libraries, referenced_names, reverse_index, comments, occurrences_by_unit, written_subtypes };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.colors(unit)
    }

    fn color_presentation(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<bool> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.has_flutter_color().then(|| project.requires_const(unit, offset))
    }

    fn augmentation_lenses(&mut self, documents: &DocumentStore, uri: &str, augmented: bool, augmentations: bool) -> Vec<(Span, &'static str, String, Span)> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project
            .augmentation_lenses(unit, augmented, augmentations)
            .into_iter()
            .filter_map(|(s, t, u, target)| Some((s, t, project.unit_uri(u)?, target)))
            .collect()
    }

    fn augmentation_neighbor(&mut self, documents: &DocumentStore, uri: &str, offset: usize, next: bool) -> Option<(String, Span)> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.augmentation_neighbor(unit, offset, next)?;
        Some((project.unit_uri(u)?, s))
    }

    fn upper(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<(String, Span)> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.upper(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn type_definition(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<(String, Span)> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.type_definition(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn closing_labels(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, String)> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.closing_labels(unit)
    }

    fn hints(&mut self, documents: &DocumentStore, uri: &str) -> Vec<crate::Hint> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.hints(unit)
    }

    fn semantic_tokens(&mut self, documents: &DocumentStore, uri: &str, multiline: bool, range: Option<(usize, usize)>) -> Option<Vec<u32>> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let highlights = project.highlights(unit);
        let text = documents.get(uri)?;
        let table = documents.lines(uri)?;
        Some(crate::semantic_tokens::encode(text, table, &highlights, multiline, range))
    }

    fn prepare_calls(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<crate::CallItem> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let e = project.call_target(unit, offset)?;
        project.call_item(e)
    }

    fn calls(&mut self, documents: &DocumentStore, uri: &str, offset: usize, name: &str, constructor: bool, received: bool) -> Vec<(crate::CallItem, Vec<Span>)> {
        let Some(project) = self.project(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        let Some((e, _)) = project.item_target(unit, offset, name, constructor) else { return Vec::new() };
        if received {
            // Only executables receive calls.
            if !matches!(e, crate::call_hierarchy::CallElem::Function(_) | crate::call_hierarchy::CallElem::Local(..)) {
                return Vec::new();
            }
            project.incoming_calls(e)
        } else {
            project.outgoing_calls(unit, offset)
        }
    }

    fn prepare_hierarchy(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<crate::TypeItem> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (c, ty) = project.hierarchy_target(unit, offset)?;
        project.type_item(c, ty)
    }

    fn hierarchy(&mut self, documents: &DocumentStore, uri: &str, reference: &str, anchor: Option<(&str, &[usize])>, supertypes: bool) -> Option<Vec<crate::TypeItem>> {
        let mut project = self.project(documents, uri)?;
        if supertypes { project.reference_supertypes(reference, anchor) } else { project.reference_subtypes(reference) }
    }

    fn set_completion_max(&mut self, max: usize) {
        self.completion_max = max;
    }

    fn set_not_imported_in_completion(&mut self, yes: bool) {
        self.not_imported_in_completion = yes;
    }

    fn completion(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<crate::Completion> {
        let text = documents.get(uri)?;
        let features = self.syntactic.features(uri, text);
        self.sdk.as_ref()?;
        let self_max = self.completion_max;
        let not_imported = self.not_imported_in_completion;
        let SemanticAnalyzer { sdk: Some(sdk), known, session, incremental, .. } = self else {
            return None;
        };
        let sdk: &SdkLayout = sdk;
        let indices = crate::completion::Indices { known };
        // The retained state of the library (that of the document's text), for the
        // incremental path (docs/LSP-ESPECIFICACAO.md §16.7); loaded and retained
        // here if missing, so that the following keystrokes find it.
        let file = crate::project::uri_file(uri)?;
        let mut retained = if *incremental {
            session.get(crate::session::Scope::Library(file), documents, || crate::project::load_library(sdk, documents, uri))
        } else {
            None
        };
        crate::completion::completion(crate::completion::CompletionInput { sdk, indices, documents, uri, text, offset, features, max: self_max, not_imported }, retained.as_deref_mut())
    }
}
