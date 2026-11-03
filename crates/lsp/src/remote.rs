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
    /// Hexadecimal encoded index (empty: none could be made).
    Records(String),
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
    match *call {
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
        Call::Records { .. } | Call::Actions { .. } => Answer::Unavailable,
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
        let answer = records_over(&mut project, files, root);
        return (project, Some(answer));
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
        project.query.infer_libraries(&missing);
        project.libraries.extend(missing);
    }
    if matches!(request.call, Call::Colors) {
        let colors = project.colors(unit);
        return (project, Some(Answer::Colors(colors)));
    }
    analyzer.session.lend(project);
    let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(analyzer, documents, &request.uri, &request.call)
    }));
    // The program goes back even when the query panicked (the session never
    // drops a lent program).
    let project = analyzer
        .session
        .take_lent()
        .expect("the session gives the lent program back");
    (project, answer.ok())
}

/// The encoded records of the libraries of `files` in `project`, inferring
/// the bodies of those it lacks; empty when one of `files` is not a unit of
/// the program (the server then records it in a child).
fn records_over(
    project: &mut crate::project::Project,
    files: &[std::path::PathBuf],
    root: &std::path::Path,
) -> Answer {
    let mut libraries = Vec::new();
    for f in files {
        let Some(u) = project.file_unit(f) else {
            return Answer::Records(String::new());
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
        project.query.infer_libraries(&missing);
        project.libraries.extend(missing);
    }
    let bytes = crate::reference_index::records_of_project(project, &libraries, root);
    Answer::Records(crate::typed::to_hex(&bytes))
}
