//! Queries the resident analysis process answers over the program it keeps
//! (docs/LSP.md, "Décima sétima rodada").
//!
//! The server used to load the document's library into its own semantic
//! session for `hover`, `definition`, `prepareRename` and the other library
//! queries: on `new_sali/frontend` that is ~376 MiB, next to the resident's
//! ~540 MiB that hold the same libraries. Now the server sends each library
//! query, with the open documents, to the resident ([`Call`] in a
//! [`QueryRequest`]); the resident lends its last analysis — the program with
//! the editor's texts, the outline and the bodies of the open documents'
//! libraries — to a [`crate::SemanticAnalyzer`] of its own, whose session
//! serves that program ([`crate::session::Session::lend`]), and runs the same
//! method the server would. The answer comes back as an [`Answer`].
//!
//! The resident's SDK is the engine's (with DDC's patches); a member a patch
//! replaced answers with the declaration the analyzer sees
//! ([`dartforge_elements::model::FunctionElement::public_declaration`]): the
//! same location, signature and documentation.
//!
//! The resident answers [`Answer::Unavailable`] when its program is not of the
//! current texts (an analysis is still to come, or the package is another's)
//! and no analysis made it current in time; the server then answers as before.

use crate::{DocumentStore, Hover, SemanticAnalyzer};
use dartforge_diagnostics::Span;

/// One library query, as the [`crate::Analyzer`] method that answers it
/// receives it (the document is [`QueryRequest::uri`]).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum Call {
    /// Document symbols with metadata resolved against the current overlays.
    Symbols,
    Hover {
        offset: usize,
    },
    Definition {
        offset: usize,
    },
    TypeDefinition {
        offset: usize,
    },
    Signature {
        offset: usize,
        auto: bool,
    },
    Highlights {
        offset: usize,
    },
    PrepareRename {
        offset: usize,
    },
    ClosingLabels,
    Hints,
    SemanticTokens {
        multiline: bool,
        range: Option<(usize, usize)>,
    },
    PrepareCalls {
        offset: usize,
    },
    PrepareHierarchy {
        offset: usize,
    },
    Upper {
        offset: usize,
    },
    Colors,
    ColorPresentation {
        offset: usize,
    },
    /// Completion at `offset`, with the server's `maxCompletionItems` and
    /// whether names not imported are offered.
    Completion {
        offset: usize,
        max: usize,
        not_imported: bool,
    },
    /// `codeAction` over `start..end` with the typed diagnostics published
    /// for the document.
    Actions {
        start: usize,
        end: usize,
        published: Vec<dartforge_diagnostics::Diagnostic>,
    },
    /// The reference index records of the libraries of `files` (canonical
    /// keys), encoded ([`crate::reference_index::ReferenceIndex::encode`]).
    Records {
        files: Vec<std::path::PathBuf>,
    },
}

/// A [`Call`] with the open documents it sees.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct QueryRequest {
    /// The SDK's `lib/` (the resident's SDK view for the analyzer it runs).
    pub(crate) sdk_lib: std::path::PathBuf,
    pub(crate) uri: String,
    /// Every open document: URI, version and text.
    pub(crate) documents: Vec<(String, i32, String)>,
    pub(crate) call: Call,
}

/// What the resident answered.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum Answer {
    /// Its program is not of the documents' texts, or it has no program.
    Unavailable,
    /// Resolved document outline over the current resident program.
    Symbols(Vec<serde_json::Value>),
    /// Intermediate reply: the retained program lacks current query inputs.
    /// The same query remains pending while the worker reloads its open files.
    NeedsAnalysis,
    Hover(Option<Hover>),
    Definition(Option<(String, Option<Span>)>),
    Location(Option<(String, Span)>),
    Signature(Option<crate::Signature>),
    Spans(Option<Vec<Span>>),
    Prepare(Result<Option<(Span, String)>, String>),
    Labels(Vec<(Span, String)>),
    Hints(Vec<crate::Hint>),
    Tokens(Option<Vec<u32>>),
    CallItem(Option<crate::CallItem>),
    TypeItem(Option<crate::TypeItem>),
    Colors(Vec<(Span, [u8; 4])>),
    Flag(Option<bool>),
    Actions(Vec<crate::CodeAction>),
    Completion(Option<crate::Completion>),
    /// Encoded index bytes (empty: none could be made). The resident transport
    /// sends these after its JSON header, without hexadecimal copies.
    Records(Vec<u8>),
}

/// The resident answers library queries (`DARTFORGE_LSP_RESIDENT_QUERIES`, on
/// by default; `0` keeps them in the server's own session).
pub(crate) fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("DARTFORGE_LSP_RESIDENT_QUERIES").map_or(true, |v| v.trim() != "0")
    })
}

/// How long the server waits for the resident's answer before it answers in
/// its own session (`DARTFORGE_LSP_QUERY_WAIT_MS`): the resident may be in
/// the middle of an analysis, or loading its first program.
pub(crate) fn wait_limit() -> std::time::Duration {
    let ms = std::env::var("DARTFORGE_LSP_QUERY_WAIT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(15_000);
    std::time::Duration::from_millis(ms)
}

/// The documents of a request, as the analyzer receives them.
pub(crate) fn documents_of(request: &QueryRequest) -> DocumentStore {
    let mut documents = DocumentStore::new();
    for (uri, version, text) in &request.documents {
        documents.open(uri.clone(), *version, text.clone());
    }
    documents
}

/// Runs `call` on `analyzer` (the resident's, whose session holds the lent
/// program) for the document `uri`.
pub(crate) fn run(
    analyzer: &mut SemanticAnalyzer,
    documents: &DocumentStore,
    uri: &str,
    call: &Call,
) -> Answer {
    use crate::Analyzer;
    let text = documents.get(uri).unwrap_or("").to_string();
    if let Call::Actions {
        start,
        end,
        published,
    } = call
    {
        return Answer::Actions(analyzer.actions(documents, uri, *start, *end, published));
    }
    if let Call::Completion {
        offset,
        max,
        not_imported,
    } = *call
    {
        analyzer.set_completion_max(max);
        analyzer.set_not_imported_in_completion(not_imported);
        return Answer::Completion(analyzer.completion(documents, uri, offset));
    }
    match *call {
        Call::Symbols => Answer::Symbols(analyzer.symbols_in(documents, uri)),
        Call::Hover { offset } => {
            Answer::Hover(analyzer.hover_in_workspace(uri, &text, offset, documents))
        }
        Call::Definition { offset } => {
            Answer::Definition(analyzer.definition_in_workspace(uri, &text, offset, documents))
        }
        Call::TypeDefinition { offset } => {
            Answer::Location(analyzer.type_definition(documents, uri, offset))
        }
        Call::Signature { offset, auto } => {
            Answer::Signature(analyzer.signature(documents, uri, offset, auto))
        }
        Call::Highlights { offset } => {
            Answer::Spans(analyzer.document_highlights(documents, uri, offset))
        }
        Call::PrepareRename { offset } => {
            Answer::Prepare(analyzer.prepare_rename(documents, uri, offset))
        }
        Call::ClosingLabels => Answer::Labels(analyzer.closing_labels(documents, uri)),
        Call::Hints => Answer::Hints(analyzer.hints(documents, uri)),
        Call::SemanticTokens { multiline, range } => {
            Answer::Tokens(analyzer.semantic_tokens(documents, uri, multiline, range))
        }
        Call::PrepareCalls { offset } => {
            Answer::CallItem(analyzer.prepare_calls(documents, uri, offset))
        }
        Call::PrepareHierarchy { offset } => {
            Answer::TypeItem(analyzer.prepare_hierarchy(documents, uri, offset))
        }
        Call::Upper { offset } => Answer::Location(analyzer.upper(documents, uri, offset)),
        Call::Colors => Answer::Colors(analyzer.colors(documents, uri)),
        Call::ColorPresentation { offset } => {
            Answer::Flag(analyzer.color_presentation(documents, uri, offset))
        }
        // Answered over the lent project directly ([`serve`]), or above.
        Call::Records { .. } | Call::Actions { .. } | Call::Completion { .. } => {
            Answer::Unavailable
        }
    }
}

/// What the resident keeps for its queries between them: the analyzer that
/// runs them (with its SDK) and how the index keys a patched member.
#[derive(Default)]
pub(crate) struct Served {
    analyzer: Option<(std::path::PathBuf, SemanticAnalyzer)>,
    patch_keys: Option<bool>,
    /// Queries answered (measurement).
    pub(crate) answered: u64,
}

/// Answers `request` over what `kept` holds; `None` when its program is not
/// of the request's texts (the caller waits for the next analysis, or gives
/// up with [`Answer::Unavailable`]).
pub(crate) fn serve(
    kept: &mut dartforge_parity::analysis::Resident,
    served: &mut Served,
    cache: &mut crate::reference_index::ResidentCache,
    request: &QueryRequest,
) -> Option<Answer> {
    let file = crate::project::uri_file(&request.uri)?;
    let root = crate::project::project_root(&file);
    if served
        .analyzer
        .as_ref()
        .is_none_or(|(sdk, _)| *sdk != request.sdk_lib)
    {
        let sdk = crate::semantic::load_sdk(&request.sdk_lib).ok()?;
        served.analyzer = Some((request.sdk_lib.clone(), SemanticAnalyzer::new(Some(sdk))));
        served.patch_keys = None;
    }
    let patch_keys = *served
        .patch_keys
        .get_or_insert_with(|| crate::reference_index::analyzer_view_has_patches(&request.sdk_lib));
    let (_, analyzer) = served.analyzer.as_mut()?;
    let documents = documents_of(request);
    // What a query that panicked left lent (its program is gone with the loan).
    drop(analyzer.session.take_lent());
    let answer = kept
        .lend_world(|lent| {
            let (project, loan) =
                crate::reference_index::project_of_loan(lent, root.clone(), cache, patch_keys);
            let (project, answer) = answer_over(project, analyzer, &documents, request, &root);
            (
                crate::reference_index::loan_of_project(project, loan, cache),
                answer,
            )
        })
        .flatten();
    if answer.is_some() {
        served.answered += 1;
    }
    answer
}

/// [`serve`] over the lent `project`.
fn answer_over(
    mut project: crate::project::Project,
    analyzer: &mut SemanticAnalyzer,
    documents: &DocumentStore,
    request: &QueryRequest,
    root: &std::path::Path,
) -> (crate::project::Project, Option<Answer>) {
    project.query.display_reference = analyzer.display_reference;
    // Every open document the program has must have its text: a later edit
    // (of this package or another's) is not in it yet.
    for (uri, _, text) in &request.documents {
        if let Some(u) = project.uri_unit(uri)
            && project.program().unit(u).source != *text
        {
            return (project, None);
        }
    }
    let Some(unit) = project.uri_unit(&request.uri) else {
        return (project, None);
    };
    if let Call::Records { files } = &request.call {
        // Closed dependents still need fresh semantic relations after an API
        // edit. Eagerly identifying every token there only builds optional
        // query caches; the queried and open files keep those shortcuts.
        let identifier_files: Vec<_> = std::iter::once(request.uri.as_str())
            .chain(request.documents.iter().map(|(uri, _, _)| uri.as_str()))
            .filter_map(|uri| project.uri_unit(uri))
            .filter_map(|unit| project.program().unit(unit).path.as_deref())
            .map(dartforge_elements::generated::key)
            .collect();
        let answer = records_over(&mut project, files, root, &identifier_files);
        return (project, Some(answer));
    }
    if matches!(request.call, Call::Symbols) {
        let symbols = crate::symbol_metadata::for_project(&project, unit);
        return (project, Some(Answer::Symbols(symbols)));
    }
    let lib = project.program().unit(unit).library;
    let mut wanted = vec![lib];
    if matches!(request.call, Call::Colors) {
        // The libraries of the color constants (Flutter's `Colors`) need their
        // bodies.
        wanted.extend(project.color_libraries(unit));
    }
    let mut missing: Vec<_> = wanted
        .into_iter()
        .filter(|l| !project.libraries.contains(l))
        .collect();
    missing.sort();
    missing.dedup();
    if !missing.is_empty() {
        project.infer_libraries(&missing);
        project.libraries.extend(missing);
    }
    if matches!(request.call, Call::Colors) {
        let colors = project.colors(unit);
        return (project, Some(Answer::Colors(colors)));
    }
    analyzer.session.lend(project);
    // A panic goes up to the loan, which drops the resident's program: a query
    // that swaps a unit (completion) may have left it half swapped.
    let answer = run(analyzer, documents, &request.uri, &request.call);
    let project = analyzer
        .session
        .take_lent()
        .expect("the session gives the lent program back");
    (project, Some(answer))
}

/// The encoded records of the libraries of `files` in `project`, inferring
/// the bodies of those it lacks; empty when one of `files` is not a unit of
/// the program (the server then records it in a child).
fn records_over(
    project: &mut crate::project::Project,
    files: &[std::path::PathBuf],
    root: &std::path::Path,
    identifier_files: &[std::path::PathBuf],
) -> Answer {
    let mut libraries = Vec::new();
    for f in files {
        let Some(u) = project.file_unit(f) else {
            return Answer::Records(Vec::new());
        };
        let lib = project.program().unit(u).library;
        if !libraries.contains(&lib) {
            libraries.push(lib);
        }
    }
    let mut missing: Vec<_> = libraries
        .iter()
        .copied()
        .filter(|l| !project.libraries.contains(l))
        .collect();
    missing.sort();
    missing.dedup();
    if !missing.is_empty() {
        project.infer_libraries(&missing);
        project.libraries.extend(missing);
    }
    let bytes =
        crate::reference_index::records_of_project(project, &libraries, root, identifier_files);
    Answer::Records(bytes)
}

#[cfg(test)]
mod record_cache_tests {
    use super::*;
    use crate::reference_index::{IdentFact, ReferenceIndex};
    use std::path::{Path, PathBuf};

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dependent_records_keep_relations_without_closed_identifier_caches() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/record-query-cache-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(directory.join("sdk/lib/core")).unwrap();
        std::fs::create_dir_all(directory.join("project/lib")).unwrap();
        let fixture = Fixture(dartforge_elements::generated::key(
            &std::fs::canonicalize(directory).unwrap(),
        ));
        let base = &fixture.0;
        std::fs::write(
            base.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(base.join("sdk/lib/core/core.dart"), "class Object {} class Type {} class Function {} class num {} class int extends num {} class String {} class bool {} class Null {} void print(Object? value) {}").unwrap();
        let root = base.join("project");
        std::fs::write(root.join("pubspec.yaml"), "name: records\n").unwrap();
        let sources = [
            (
                "api.dart",
                "class Box { int value = 0; void work(int n, {int named = 0}) {} } Box makeBox() => Box();",
            ),
            (
                "caller.dart",
                "import 'api.dart'; part 'shared.dart'; void call() { final box = makeBox(); box.work(1, named: 2); print(box.value); }",
            ),
            (
                "closed.dart",
                "import 'api.dart'; void closed() { final box = makeBox(); box.work(3); print(box.value); }",
            ),
            (
                "shared.dart",
                "part of 'caller.dart'; void shared() { final box = makeBox(); box.work(4); }",
            ),
        ];
        let mut documents = DocumentStore::new();
        let mut paths = Vec::new();
        for (name, text) in sources {
            let path = root.join("lib").join(name);
            std::fs::write(&path, text).unwrap();
            if name == "api.dart" || name == "caller.dart" {
                documents.open(
                    url::Url::from_file_path(&path).unwrap().to_string(),
                    1,
                    text.to_owned(),
                );
            }
            paths.push(path);
        }
        let sdk = crate::semantic::load_sdk(&base.join("sdk/lib")).unwrap();
        let uri = url::Url::from_file_path(&paths[1]).unwrap().to_string();
        let mut full = crate::reference_index::build(&sdk, &documents, &uri).unwrap();
        let before = full.encode();
        let project = crate::project::load_project(&sdk, &documents, &uri).unwrap();
        let request = QueryRequest {
            sdk_lib: sdk.root.clone(),
            uri,
            documents: documents
                .uris()
                .map(|uri| (uri.to_owned(), 1, documents.get(uri).unwrap().to_owned()))
                .collect(),
            call: Call::Records {
                files: paths.clone(),
            },
        };
        let mut analyzer = SemanticAnalyzer::new(Some(sdk));
        let (_, answer) = answer_over(project, &mut analyzer, &documents, &request, &root);
        let Some(Answer::Records(bytes)) = answer else {
            panic!("resident did not record the libraries")
        };
        let part = ReferenceIndex::decode(&bytes).unwrap();
        for path in &paths {
            assert_eq!(
                full.describe(path),
                part.describe(path),
                "relations changed: {path:?}"
            );
            let record = part.file(path).unwrap();
            if path == &paths[0] || path == &paths[1] {
                assert!(!record.idents.is_empty(), "open file lost its cache");
                assert!(
                    !record.prepares.is_empty(),
                    "open file lost rename preparation"
                );
            } else {
                assert!(
                    record.idents.is_empty()
                        && record.facts.is_empty()
                        && record.prepares.is_empty()
                        && record.prepare_facts.is_empty(),
                    "closed file eagerly rebuilt optional caches: {path:?}"
                );
            }
        }
        let targets: Vec<_> = full
            .records()
            .filter(|record| !record.frozen)
            .flat_map(|record| record.facts.iter())
            .filter_map(|fact| {
                if let IdentFact::Target(target) = fact {
                    Some(target.clone())
                } else {
                    None
                }
            })
            .collect();
        let mut refreshed = ReferenceIndex::decode(&before).unwrap();
        refreshed.merge(part);
        let mut refreshed = ReferenceIndex::decode(&refreshed.encode()).unwrap();
        let mut answered = 0;
        for target in targets {
            let expected = full.references(&target);
            let actual = refreshed.references(&target);
            match (expected, actual) {
                (Ok(expected), Ok(actual)) => {
                    assert_eq!(actual, expected);
                    answered += 1;
                }
                (Err(expected), Err(actual)) => {
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"))
                }
                (expected, actual) => {
                    panic!("reference availability changed: {expected:?} -> {actual:?}")
                }
            }
        }
        assert!(answered >= 3, "no meaningful reference targets compared");
    }
}
