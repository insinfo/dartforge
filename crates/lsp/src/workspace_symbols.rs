//! `workspace/symbol` like the Dart 3.6.2 server (docs/LSP-ESPECIFICACAO.md
//! §5): the `WorkspaceSymbolHandler` over the analyzer's `FindDeclarations`.
//!
//! * the libraries come in `OwnedFiles` order: first the added files (the
//!   project's), then those reached during analysis, then those known through
//!   `discoverAvailableFiles` (the SDK libraries and the `lib/` of the packages
//!   in `package_config.json`); a part is not a library and is left out (its
//!   content comes in through the owning library);
//! * each library goes through the `ElementNameUnion.contains` pre-filter;
//! * per unit, in the order of `_FindCompilationUnitDeclarations`: top-level
//!   accessors, classes (with their accessors, constructors, fields and
//!   methods), enums, mixins, extensions, extension types, functions,
//!   typedefs and variables; only non-synthetic elements;
//! * the name matches through the `FuzzyMatcher` (`MatchStyle.TEXT`) with
//!   score ≥ 0, without sorting; collection stops at 500;
//! * the item: the name with `()`/`(…)` from the executable's
//!   `getDisplayString()`, the kind from `declarationKindToSymbolKind`, the
//!   element's code range and the `containerName` of a class or mixin.
//!
//! The index is only syntactic (the parser, without resolution) and is
//! cached per file; a library's index is rebuilt when it or one of its
//! parts changes.
//!
//! The known files also have the `build_runner` outputs that the libraries
//! import (`package:x/a.template.dart`, which only exists under
//! `.dart_tool/build/generated`): the analyzer's `PackageConfigWorkspace`
//! resolves those URIs to the generated files, and each `FileState` it
//! creates is a known file. They come in the order dependencies reach them
//! from the added files, before the remaining available libraries.

use crate::fuzzy_matcher::{Matcher, Style};
use dartforge_frontend::ast::{
    self, Annotation, Ast, DeclKind, DirectiveKind, FunctionKind, MemberKind, TypeKind,
};
use dartforge_frontend::comments::Comments;
use dartforge_intern::Interner;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

mod context_order;
mod uri_resolution;
pub(crate) use context_order::{DriverOrder, root_order};
use uri_resolution::dependency_target;

/// The handler's result limit.
pub(crate) const LIMIT: usize = 500;

// -- ElementNameUnion ---------------------------------------------------------

/// `ElementNameUnion`: for each letter position (up to 63), the letters that
/// some name of the library has there; `mask[0]` is the longest length.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NameUnion {
    mask: [u32; 64],
}

impl NameUnion {
    const MAX: u32 = 63;

    fn empty() -> Self {
        Self { mask: [0; 64] }
    }

    fn add(&mut self, name: &str) {
        if self.mask[0] >= Self::MAX {
            return;
        }
        let mut index = 1usize;
        for c in name.encode_utf16() {
            if (0x41..=0x5A).contains(&c) {
                self.mask[index] |= 1 << (c - 0x41);
                index += 1;
            } else if (0x61..=0x7A).contains(&c) {
                self.mask[index] |= 1 << (c - 0x61);
                index += 1;
            }
            if index > Self::MAX as usize {
                self.mask[0] = Self::MAX;
                return;
            }
        }
        let length = (index - 1) as u32;
        if self.mask[0] < length {
            self.mask[0] = length;
        }
    }

    fn contains(&self, pattern: &str) -> bool {
        let max = self.mask[0];
        if max >= Self::MAX {
            return true;
        }
        let mut index = 1usize;
        for c in pattern.encode_utf16() {
            let m: u32 = if (0x41..=0x5A).contains(&c) {
                1 << (c - 0x41)
            } else if (0x61..=0x7A).contains(&c) {
                1 << (c - 0x61)
            } else {
                continue;
            };
            loop {
                if index > max as usize {
                    return false;
                }
                let x = self.mask[index];
                index += 1;
                if x & m != 0 {
                    break;
                }
            }
        }
        true
    }
}

// -- Type summary (for `getDisplayString()`) ----------------------------------

/// What matters about a written type to know where the first `(` of the
/// executable's `getDisplayString()` is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Summary {
    /// `Name<args>` (the last segment of `p.Name`).
    Name {
        name: String,
        args: Vec<Summary>,
    },
    /// `R Function(…)`, with the written return.
    Function(Option<Box<Summary>>),
    /// `(int, String)`.
    Registration,
    Void,
    /// Type parameter `i` of the `typedef` being expanded.
    Parameter(usize),
}

/// A `typedef`: the aliased type (the type parameters become
/// [`Summary::Parameter`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Alias {
    target: Summary,
}

/// Where the `(` appears in the displayed text of a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paren {
    None,
    Contains,
    /// The text starts with `(` (a record).
    StartsWith,
}

/// The types declared in a library: `Some` for a `typedef`, `None` for a
/// class, mixin, enum or extension type (which hides a homonymous `typedef`
/// from another library).
type LocalTypes = HashMap<String, Option<Alias>>;

fn paren(
    r: &Summary,
    ctx: &[Paren],
    locals: &LocalTypes,
    global: &HashMap<String, Alias>,
    depth: u32,
) -> Paren {
    match r {
        Summary::Void => Paren::None,
        Summary::Registration => Paren::StartsWith,
        Summary::Parameter(i) => ctx.get(*i).copied().unwrap_or(Paren::None),
        // `R Function(…)`: starts with `(` only when the return does.
        Summary::Function(return_) => match return_ {
            Some(x) if paren(x, ctx, locals, global, depth) == Paren::StartsWith => {
                Paren::StartsWith
            }
            _ => Paren::Contains,
        },
        Summary::Name { name, args } => {
            let arguments: Vec<Paren> = args
                .iter()
                .map(|a| paren(a, ctx, locals, global, depth))
                .collect();
            if depth < 16 {
                let alias = match locals.get(name) {
                    Some(local) => local.as_ref(),
                    None => global.get(name),
                };
                // `getDisplayString()` without `preferTypeAlias` shows the aliased type.
                if let Some(a) = alias {
                    return paren(&a.target, &arguments, locals, global, depth + 1);
                }
            }
            if arguments.iter().any(|p| *p != Paren::None) {
                Paren::Contains
            } else {
                Paren::None
            }
        }
    }
}

fn summarize(ast: &Ast, names: &Interner, t: ast::TypeId, parameters: &[String]) -> Summary {
    let ty = ast.ty(t);
    match &ty.kind {
        TypeKind::Void => Summary::Void,
        TypeKind::Record { .. } => Summary::Registration,
        TypeKind::Function { return_type, .. } => {
            Summary::Function(return_type.map(|r| Box::new(summarize(ast, names, r, parameters))))
        }
        TypeKind::Named { name, args } => {
            let name_ = name
                .last()
                .map(|n| names.resolve(n.sym).to_string())
                .unwrap_or_default();
            if name.len() == 1
                && args.is_empty()
                && let Some(i) = parameters.iter().position(|p| *p == name_)
            {
                return Summary::Parameter(i);
            }
            Summary::Name {
                name: name_,
                args: args
                    .iter()
                    .map(|a| summarize(ast, names, *a, parameters))
                    .collect(),
            }
        }
    }
}

// -- Declarations ---------------------------------------------------------------

/// The analyzer's `DeclarationKind` (those used by `_getSearchElementKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Class,
    ClassAlias,
    Enum,
    Mixin,
    Extension,
    ExtensionType,
    Constructor,
    EnumConstant,
    Field,
    Function,
    Method,
    Getter,
    Setter,
    Typedef,
    Variable,
}

impl Kind {
    /// The preferences of `declarationKindToSymbolKind`.
    fn preferences(self) -> &'static [u64] {
        match self {
            Kind::Class
            | Kind::ClassAlias
            | Kind::Extension
            | Kind::ExtensionType
            | Kind::Mixin
            | Kind::Typedef => &[5],
            Kind::Constructor => &[9],
            Kind::Enum => &[10],
            Kind::EnumConstant => &[22, 10],
            Kind::Field => &[8],
            Kind::Function => &[12],
            Kind::Getter | Kind::Setter => &[7],
            Kind::Method => &[6],
            Kind::Variable => &[13],
        }
    }
}

/// What the element's `getDisplayString()` needs for the suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Signature {
    /// Not an executable.
    None,
    /// `A A.n(…)`: the first `(` is the list's.
    List { empty: bool },
    /// `R get x`: only the return can have a `(`.
    Getter {
        return_: Option<Summary>,
        augment: bool,
    },
    /// `R name<T>(…)`.
    Executable {
        return_: Option<Summary>,
        empty: bool,
        augment: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    name: String,
    kind: Kind,
    /// `className ?? mixinName`, shared by the containing declaration's members.
    container: Option<Arc<str>>,
    signature: Signature,
    /// The code range in LSP positions of the file.
    start: (u32, u32),
    end: (u32, u32),
}

#[derive(Debug, PartialEq, Eq)]
struct Unit {
    uri: String,
    declarations: Vec<Declaration>,
}

#[derive(Debug, PartialEq, Eq)]
struct Library {
    union: NameUnion,
    units: Vec<Unit>,
    types: LocalTypes,
    /// The URIs its units import, with the unit's file.
    imports: Vec<Dependency>,
    /// Exports are separate from imports: bootstrap and cycle discovery
    /// materialize their lazy lists in different orders.
    exports: Vec<Dependency>,
}

/// FileState builds every configurable URI, even when only the primary is
/// selected. The analysis server declares no conditional variables, including
/// dart.library.*: absent variables match neither false nor the empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Dependency {
    unit: Arc<PathBuf>,
    primary: Option<String>,
    alternatives: Vec<String>,
}

#[derive(Debug)]
struct SdkFiles {
    root: PathBuf,
    layout: dartforge_elements::sdk::SdkLayout,
    bootstrap: Vec<PathBuf>,
    available: Vec<PathBuf>,
}

/// A driver's package catalog and generated workspace. Dependencies keep the
/// referring driver, including when they are shared by multiple contexts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Context {
    root: PathBuf,
    /// Session driver-map iteration rank; absent for standalone catalogs.
    driver_rank: Option<usize>,
    packages: Vec<(String, PathBuf)>,
    generated: Vec<(String, PathBuf, PathBuf)>,
    generated_files: Vec<(PathBuf, PathBuf)>,
    /// Loaded package and generated mapping roots are already lexical keys.
    /// Standalone catalogs keep the general resolver's normalization.
    normalized_paths: bool,
}

impl Context {
    /// AnalysisDriver.addFile checks FileSystemState.hasUri: mapping this
    /// physical path to a canonical URI and back must preserve the path.
    /// A shadowed non-lib source is not added merely because it is on disk.
    fn has_uri(&self, path: &Path) -> bool {
        let path = dartforge_elements::generated::key(path);
        let uri = match self.identity(&path) {
            FileIdentity::Package(name, relative) => format!(
                "package:{name}/{}",
                relative
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            ),
            FileIdentity::File(source) => {
                let Ok(uri) = url::Url::from_file_path(source) else {
                    return false;
                };
                uri.to_string()
            }
        };
        dependency_target(
            &path,
            &uri,
            None,
            &self.packages,
            &self.generated,
            &self.generated_files,
        )
        .is_some_and(|resolved| dartforge_elements::generated::key(&resolved) == path)
    }

    /// Loads the nearest package configuration, which can be inherited by a
    /// context with no configuration of its own (for example a BUILD.gn root).
    pub(crate) fn load(root: &Path) -> Self {
        let config_root = root
            .ancestors()
            .find(|p| p.join(".dart_tool/package_config.json").is_file());
        let locations = config_root
            .map(crate::project::packages_in_analysis_order)
            .unwrap_or_default();
        let packages: Vec<_> = locations
            .iter()
            // Package configurations often contain long relative paths with
            // `..`. URI discovery compares every import against this catalog;
            // normalize once rather than rebuilding each root per comparison.
            .map(|p| (p.name.clone(), dartforge_elements::generated::key(&p.lib)))
            .collect();
        let generated = config_root
            .into_iter()
            .flat_map(|root| {
                let output = root.join(".dart_tool/build/generated");
                packages.iter().filter_map(move |(name, lib)| {
                    let generated = output.join(name).join("lib");
                    generated.is_dir().then(|| {
                        (
                            name.clone(),
                            lib.clone(),
                            dartforge_elements::generated::key(&generated),
                        )
                    })
                })
            })
            .collect();
        let generated_files = config_root
            .into_iter()
            .flat_map(|root| {
                let output = root.join(".dart_tool/build/generated");
                locations
                    .iter()
                    .filter(move |p| p.root.starts_with(root))
                    .filter_map(move |p| {
                        let generated = output.join(&p.name);
                        generated.is_dir().then(|| {
                            (
                                dartforge_elements::generated::key(&p.root),
                                dartforge_elements::generated::key(&generated),
                            )
                        })
                    })
            })
            .collect();
        Self {
            root: root.to_path_buf(),
            driver_rank: None,
            packages,
            generated,
            generated_files,
            normalized_paths: true,
        }
    }

    /// SourceFactory.pathToUri: a package URI identifies a library across
    /// different physical versions of the same package in different drivers.
    fn identity(&self, path: &Path) -> FileIdentity {
        for (name, _, output) in &self.generated {
            if let Ok(relative) = path.strip_prefix(output) {
                return FileIdentity::Package(name.clone(), relative.to_path_buf());
            }
        }
        for (name, folder) in &self.packages {
            let folder = uri_resolution::catalog_path(folder, self.normalized_paths);
            if let Ok(relative) = path.strip_prefix(folder.as_ref()) {
                return FileIdentity::Package(name.clone(), relative.to_path_buf());
            }
        }
        for (source, output) in &self.generated_files {
            if let Ok(relative) = path.strip_prefix(output) {
                return FileIdentity::File(source.join(relative));
            }
        }
        FileIdentity::File(path.to_path_buf())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FileIdentity {
    File(PathBuf),
    Package(String, PathBuf),
}

#[derive(Debug, Clone)]
struct OwnedFile {
    context: usize,
    identity: FileIdentity,
    path: PathBuf,
}

/// OwnedFiles: added URIs keep their first owner, known URIs their most recent
/// owner. Replacing an owner preserves the URI's original insertion position.
#[derive(Debug, Default)]
struct Discovery {
    files: Vec<OwnedFile>,
    positions: HashMap<FileIdentity, usize>,
    added_count: usize,
    created: HashSet<(usize, PathBuf)>,
    /// Query resolution can add files after the response snapshot. Once a
    /// trusted snapshot resolves without new FileStates, repeated queries
    /// need no further graph traversal until the inputs change.
    snapshot_resolved: bool,
}

impl Discovery {
    fn add(&mut self, path: &Path, context: usize, contexts: &[Context], added: bool) {
        let path = dartforge_elements::generated::key(path);
        if !self.created.insert((context, path.clone())) {
            return;
        }
        let identity = contexts
            .get(context)
            .map_or_else(|| FileIdentity::File(path.clone()), |c| c.identity(&path));
        if let Some(&position) = self.positions.get(&identity) {
            if !added && position >= self.added_count {
                self.files[position] = OwnedFile {
                    context,
                    identity,
                    path,
                };
            }
        } else {
            self.positions.insert(identity.clone(), self.files.len());
            self.files.push(OwnedFile {
                context,
                identity,
                path,
            });
            if added {
                self.added_count += 1;
            }
        }
    }

    fn retain_live(
        &mut self,
        open: &HashMap<PathBuf, Fingerprint>,
        contexts: &[Context],
        disk: &DiskSnapshot,
    ) {
        self.files
            .retain(|f| open.contains_key(&f.path) || disk.state(&f.path).is_file);
        self.positions.clear();
        for (i, file) in self.files.iter().enumerate() {
            self.positions.insert(file.identity.clone(), i);
        }
        // A missing FileState can share a canonical URI with a live file in
        // another context. Forgetting that creation would let it reclaim the
        // URI on the next traversal. Keep these collision records, while
        // discarding missing imports with no live binding to avoid retaining
        // an unbounded history of invalid directives typed in the editor.
        self.created.retain(|(context, path)| {
            open.contains_key(path)
                || disk.state(path).is_file
                || self.positions.contains_key(
                    &contexts
                        .get(*context)
                        .map_or_else(|| FileIdentity::File(path.clone()), |c| c.identity(path)),
                )
        });
    }
}

fn context_for(path: &Path, contexts: &[Context]) -> usize {
    contexts
        .iter()
        .enumerate()
        .filter(|(_, c)| path.starts_with(&c.root))
        .max_by_key(|(_, c)| c.root.components().count())
        .map_or(0, |(i, _)| i)
}

/// Inputs that can change library discovery, excluding edits to bodies and
/// declaration ranges. Opening order follows AnalysisDriver.priorityFiles.
#[derive(Debug, PartialEq, Eq)]
struct DiscoveryInputs {
    added: Vec<PathBuf>,
    sdk: Option<PathBuf>,
    contexts: Vec<Context>,
    priority: Vec<PathBuf>,
    open_dependencies: Vec<OpenDependencies>,
}

#[derive(Debug, PartialEq, Eq)]
struct OpenDependencies {
    path: PathBuf,
    imports: Vec<Dependency>,
    exports: Vec<Dependency>,
}

/// An unavailable file, its size and date on disk, or the open text's size
/// and digest. Absence must be tracked for generated parts too.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fingerprint {
    Missing,
    Disk(Option<SystemTime>, u64),
    Open(usize, u64),
}

#[derive(Debug)]
struct Entry {
    /// The library's file and its parts, with their fingerprints.
    files: Vec<(PathBuf, Fingerprint)>,
    library: Option<Arc<Library>>,
    /// A library is checked once per search, even if discovery visits it again.
    checked_at: u64,
}

/// The index: the libraries already read, by path.
#[derive(Debug, Default)]
pub(crate) struct Index {
    entries: HashMap<PathBuf, Entry>,
    search_generation: u64,
    /// The SDK's bootstrap and discovery order, by `lib/`.
    sdk: Option<Arc<SdkFiles>>,
    /// The `.dart` files of each package's `lib/`, as last listed.
    listed: HashMap<PathBuf, Vec<PathBuf>>,
    /// The last search's files, with what they were listed from.
    paths: Option<(DiscoveryInputs, Discovery)>,
}

/// Reads a file (open or from disk) with its fingerprint.
type Reader<'a> = dyn Fn(&Path) -> Option<(String, Fingerprint)> + 'a;

/// Inputs shared by pre-query analysis and snapshot resolution. Each external
/// file keeps its referring catalog rather than choosing one from its path.
struct DiscoveryContext<'a> {
    catalogs: &'a [Context],
    sdk: Option<&'a SdkFiles>,
    read: &'a Reader<'a>,
    open: &'a HashMap<PathBuf, Fingerprint>,
    uri_of: &'a dyn Fn(&Path) -> String,
    trust_disk: bool,
    disk: &'a DiskSnapshot,
    relative_bases: uri_resolution::RelativeBases,
    /// Pre-query analysis and snapshot resolution share completed directive
    /// materialization, separately for each referring context and this query.
    resolved: RefCell<HashSet<(usize, PathBuf)>>,
}

/// Metadata needed by declaration validation and discovery.
#[derive(Debug, Clone)]
struct DiskState {
    is_file: bool,
    fingerprint: Option<Fingerprint>,
}

impl DiskState {
    fn read(path: &Path) -> Self {
        let metadata = std::fs::metadata(path).ok();
        Self {
            is_file: metadata.as_ref().is_some_and(std::fs::Metadata::is_file),
            fingerprint: metadata.map(|m| Fingerprint::Disk(m.modified().ok(), m.len())),
        }
    }
}

/// Metadata shared while revalidating a previous discovery. Initial builds
/// and trusted lookups need no map. Generated URI target selection stays fresh.
/// Exact OS strings avoid component hashing without conflating path aliases.
struct DiskSnapshot {
    states: Option<RefCell<HashMap<OsString, DiskState>>>,
}

impl DiskSnapshot {
    fn new(share: bool) -> Self {
        Self {
            states: share.then(RefCell::default),
        }
    }

    fn state(&self, path: &Path) -> DiskState {
        let Some(states) = &self.states else {
            return DiskState::read(path);
        };
        let mut states = states.borrow_mut();
        if let Some(state) = states.get(path.as_os_str()) {
            return state.clone();
        }
        let state = DiskState::read(path);
        states.insert(path.as_os_str().to_owned(), state.clone());
        state
    }
}

/// The fingerprint of an open text.
fn text_fingerprint(text: &str) -> Fingerprint {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    Fingerprint::Open(text.len(), h.finish())
}

/// The first code position from `pos` (skips whitespace and comments).
fn next_code(source: &str, mut pos: usize) -> usize {
    let b = source.as_bytes();
    loop {
        while pos < b.len() && b[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'/') {
            while pos < b.len() && b[pos] != b'\n' && b[pos] != b'\r' {
                pos += 1;
            }
            continue;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'*') {
            let mut level = 0usize;
            while pos < b.len() {
                if b[pos] == b'/' && b.get(pos + 1) == Some(&b'*') {
                    level += 1;
                    pos += 2;
                } else if b[pos] == b'*' && b.get(pos + 1) == Some(&b'/') {
                    level -= 1;
                    pos += 2;
                    if level == 0 {
                        break;
                    }
                } else {
                    pos += 1;
                }
            }
            continue;
        }
        return pos;
    }
}

/// What reads a unit: the text, the lines and the comments.
struct Read<'a> {
    source: &'a str,
    ast: &'a Ast,
    names: &'a Interner,
    lines: crate::utf16::LineTable,
    comments: Comments,
}

impl Read<'_> {
    fn name(&self, n: ast::Name) -> String {
        self.names.resolve(n.sym).to_string()
    }

    /// The `offset` of an `AnnotatedNode`: the documentation comment and the
    /// annotations (`AnnotatedNodeImpl.beginToken`).
    fn annotated_start(&self, start_: usize, metadata: &[Annotation]) -> usize {
        let after = match metadata.last() {
            Some(m) => next_code(self.source, m.span.end),
            None => start_,
        };
        let mut doc = self.comments.dart_doc(self.source, after);
        if doc.is_none() {
            for m in metadata.iter().rev() {
                doc = self.comments.dart_doc(self.source, m.span.start);
                if doc.is_some() {
                    break;
                }
            }
        }
        let mut start = start_;
        if let Some(m) = metadata.first() {
            start = start.min(m.span.start);
        }
        if let Some(d) = doc {
            start = start.min(d.start);
        }
        start
    }

    fn position(&self, offset: usize) -> (u32, u32) {
        self.lines.offset_position(self.source, offset)
    }

    fn declaration(
        &self,
        name: String,
        kind: Kind,
        container: Option<Arc<str>>,
        signature: Signature,
        start: usize,
        end: usize,
    ) -> Declaration {
        Declaration {
            name,
            kind,
            container,
            signature,
            start: self.position(start),
            end: self.position(end),
        }
    }

    /// The end of a variable: that of the initializer, else that of the name.
    fn variable_end(&self, v: &ast::Variable) -> usize {
        v.initializer
            .map_or(v.name.span.end, |e| self.ast.expr(e).span.end)
    }
}

/// The content of a unit for the declaration list and the union.
struct Collector<'a, 'b> {
    l: &'b Read<'a>,
    output: Vec<Declaration>,
    union: &'b mut NameUnion,
    types: &'b mut LocalTypes,
}

impl Collector<'_, '_> {
    /// `_addDeclaration` without the matching (which is done at query time).
    fn push(&mut self, d: Declaration) {
        self.output.push(d);
    }

    fn function_signature(
        &self,
        f: &ast::Function,
        augment: bool,
        type_parameters: &[String],
    ) -> Signature {
        let return_ = f
            .return_type
            .map(|t| summarize(self.l.ast, self.l.names, t, type_parameters));
        match f.kind {
            FunctionKind::Getter => Signature::Getter { return_, augment },
            FunctionKind::Setter => Signature::List {
                empty: f.parameters.as_ref().is_none_or(|p| p.is_empty()),
            },
            FunctionKind::Function | FunctionKind::Operator => Signature::Executable {
                return_,
                empty: f.parameters.as_ref().is_none_or(|p| p.is_empty()),
                augment,
            },
        }
    }

    /// The members of a class, mixin, enum, extension or extension type, in the
    /// order of `_addClasses`/`_addExtensions`: accessors, constructors, fields
    /// (`first_fields` before the declared ones) and methods.
    fn members(
        &mut self,
        members: &[ast::MemberId],
        container: Option<Arc<str>>,
        constructors: bool,
        first_fields: Vec<Declaration>,
        first_constructors: Vec<Declaration>,
    ) {
        let l = self.l;
        let ast = l.ast;
        let mut accessors = Vec::new();
        let mut ctors = first_constructors;
        let mut fields = first_fields;
        let mut methods = Vec::new();
        for &mid in members {
            let m = ast.member(mid);
            let start = l.annotated_start(m.span.start, &m.metadata);
            match &m.kind {
                MemberKind::Field(list) => {
                    for (i, v) in list.variables.iter().enumerate() {
                        let name = l.name(v.name);
                        self.union.add(&name);
                        let start_ = if i == 0 { start } else { v.name.span.start };
                        fields.push(l.declaration(
                            name,
                            Kind::Field,
                            container.clone(),
                            Signature::None,
                            start_,
                            l.variable_end(v),
                        ));
                    }
                }
                MemberKind::Method(fid) => {
                    let f = ast.function(*fid);
                    let Some(n) = f.name else { continue };
                    let name = if f.kind == FunctionKind::Operator
                        && l.names.resolve(n.sym) == "-"
                        && f.parameters.as_ref().is_some_and(|p| p.is_empty())
                    {
                        "unary-".to_string()
                    } else {
                        l.name(n)
                    };
                    self.union.add(&name);
                    let signature = self.function_signature(f, m.augment, &[]);
                    match f.kind {
                        FunctionKind::Getter => accessors.push(l.declaration(
                            name,
                            Kind::Getter,
                            container.clone(),
                            signature,
                            start,
                            m.span.end,
                        )),
                        FunctionKind::Setter => accessors.push(l.declaration(
                            name,
                            Kind::Setter,
                            container.clone(),
                            signature,
                            start,
                            m.span.end,
                        )),
                        _ => methods.push(l.declaration(
                            name,
                            Kind::Method,
                            container.clone(),
                            signature,
                            start,
                            m.span.end,
                        )),
                    }
                }
                MemberKind::Constructor(c) => {
                    if !constructors {
                        continue;
                    }
                    let name = c
                        .name
                        .map(|n| l.name(n))
                        .filter(|n| n != "new")
                        .unwrap_or_default();
                    ctors.push(l.declaration(
                        name,
                        Kind::Constructor,
                        container.clone(),
                        Signature::List {
                            empty: c.parameters.is_empty(),
                        },
                        start,
                        m.span.end,
                    ));
                }
            }
        }
        self.output.extend(accessors);
        self.output.extend(ctors);
        self.output.extend(fields);
        self.output.extend(methods);
    }

    fn unit(&mut self, unit: &ast::CompilationUnit) {
        let l = self.l;
        let ast = l.ast;
        let mut accessors = Vec::new();
        let mut classes = Vec::new();
        let mut enums = Vec::new();
        let mut mixins = Vec::new();
        let mut extensions = Vec::new();
        let mut extension_types = Vec::new();
        let mut functions = Vec::new();
        let mut aliases = Vec::new();
        let mut variables = Vec::new();
        for &did in &unit.declarations {
            let d = ast.decl(did);
            match &d.kind {
                DeclKind::Class(_) => classes.push(did),
                DeclKind::Enum(_) => enums.push(did),
                DeclKind::Mixin(_) => mixins.push(did),
                DeclKind::Extension(_) => extensions.push(did),
                DeclKind::ExtensionType(_) => extension_types.push(did),
                DeclKind::Typedef(_) => aliases.push(did),
                DeclKind::Variables(_) => variables.push(did),
                DeclKind::Function(fid) => match ast.function(*fid).kind {
                    FunctionKind::Getter | FunctionKind::Setter => accessors.push(did),
                    _ => functions.push(did),
                },
            }
        }
        for did in accessors {
            let d = ast.decl(did);
            let DeclKind::Function(fid) = d.kind else {
                continue;
            };
            let f = ast.function(fid);
            let Some(n) = f.name else { continue };
            let name = l.name(n);
            self.union.add(&name);
            let kind = if f.kind == FunctionKind::Getter {
                Kind::Getter
            } else {
                Kind::Setter
            };
            let signature = self.function_signature(f, d.augment, &[]);
            let start = l.annotated_start(d.span.start, &d.metadata);
            self.push(l.declaration(name, kind, None, signature, start, d.span.end));
        }
        for did in classes.into_iter().chain(enums).chain(mixins) {
            let d = ast.decl(did);
            let start = l.annotated_start(d.span.start, &d.metadata);
            match &d.kind {
                DeclKind::Class(c) => {
                    let name = l.name(c.name);
                    self.union.add(&name);
                    self.types.insert(name.clone(), None);
                    let kind = if c.mixin_application {
                        Kind::ClassAlias
                    } else {
                        Kind::Class
                    };
                    self.push(l.declaration(
                        name.clone(),
                        kind,
                        None,
                        Signature::None,
                        start,
                        d.span.end,
                    ));
                    self.members(
                        &c.members,
                        Some(Arc::from(name.as_str())),
                        true,
                        Vec::new(),
                        Vec::new(),
                    );
                }
                DeclKind::Enum(e) => {
                    let name = l.name(e.name);
                    self.union.add(&name);
                    self.types.insert(name.clone(), None);
                    self.push(l.declaration(
                        name,
                        Kind::Enum,
                        None,
                        Signature::None,
                        start,
                        d.span.end,
                    ));
                    // The constants are fields; the synthetic `values` only goes into the union.
                    let mut constants = Vec::new();
                    for k in &e.constants {
                        let n = l.name(k.name);
                        self.union.add(&n);
                        let ci = l.annotated_start(k.span.start, &k.metadata);
                        constants.push(l.declaration(
                            n,
                            Kind::EnumConstant,
                            None,
                            Signature::None,
                            ci,
                            k.span.end,
                        ));
                    }
                    self.union.add("values");
                    self.members(&e.members, None, true, constants, Vec::new());
                }
                DeclKind::Mixin(m) => {
                    let name = l.name(m.name);
                    self.union.add(&name);
                    self.types.insert(name.clone(), None);
                    self.push(l.declaration(
                        name.clone(),
                        Kind::Mixin,
                        None,
                        Signature::None,
                        start,
                        d.span.end,
                    ));
                    self.members(
                        &m.members,
                        Some(Arc::from(name.as_str())),
                        true,
                        Vec::new(),
                        Vec::new(),
                    );
                }
                _ => {}
            }
        }
        for did in extensions {
            let d = ast.decl(did);
            let DeclKind::Extension(x) = &d.kind else {
                continue;
            };
            if let Some(n) = x.name {
                let name = l.name(n);
                self.union.add(&name);
                let start = l.annotated_start(d.span.start, &d.metadata);
                self.push(l.declaration(
                    name,
                    Kind::Extension,
                    None,
                    Signature::None,
                    start,
                    d.span.end,
                ));
            }
            self.members(&x.members, None, false, Vec::new(), Vec::new());
        }
        for did in extension_types {
            let d = ast.decl(did);
            let DeclKind::ExtensionType(x) = &d.kind else {
                continue;
            };
            let name = l.name(x.name);
            self.union.add(&name);
            self.types.insert(name.clone(), None);
            let start = l.annotated_start(d.span.start, &d.metadata);
            self.push(l.declaration(
                name.clone(),
                Kind::ExtensionType,
                None,
                Signature::None,
                start,
                d.span.end,
            ));
            // The representation field and the primary constructor
            // (`_builtRepresentationDeclaration`) come before the declared ones.
            let container: Arc<str> = Arc::from(name.as_str());
            let field_name_ = Some(l.name(x.representation_name))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "<empty>".to_string());
            self.union.add(&field_name_);
            let field_start = x
                .representation_metadata
                .first()
                .map_or(l.ast.ty(x.representation_type).span.start, |m| m.span.start);
            let field = l.declaration(
                field_name_,
                Kind::Field,
                Some(container.clone()),
                Signature::None,
                field_start,
                x.representation_name.span.end,
            );
            let (ctor_name, ctor_start) = match x.constructor {
                Some(n) => {
                    let text = l.name(n);
                    // The node starts at the `.` of the name.
                    let before = l.source[..n.span.start].trim_end();
                    let dot = if before.ends_with('.') {
                        before.len() - 1
                    } else {
                        n.span.start
                    };
                    (if text == "new" { String::new() } else { text }, dot)
                }
                None => (String::new(), x.representation_span.start),
            };
            let ctor = l.declaration(
                ctor_name,
                Kind::Constructor,
                Some(container.clone()),
                Signature::List { empty: false },
                ctor_start,
                x.representation_span.end,
            );
            self.members(&x.members, Some(container), true, vec![field], vec![ctor]);
        }
        for did in functions {
            let d = ast.decl(did);
            let DeclKind::Function(fid) = d.kind else {
                continue;
            };
            let f = ast.function(fid);
            let Some(n) = f.name else { continue };
            let name = l.name(n);
            self.union.add(&name);
            let parameters: Vec<String> = f.type_params.iter().map(|t| l.name(t.name)).collect();
            let signature = self.function_signature(f, d.augment, &parameters);
            let start = l.annotated_start(d.span.start, &d.metadata);
            self.push(l.declaration(name, Kind::Function, None, signature, start, d.span.end));
        }
        for did in aliases {
            let d = ast.decl(did);
            let DeclKind::Typedef(t) = &d.kind else {
                continue;
            };
            let name = l.name(t.name);
            self.union.add(&name);
            let parameters: Vec<String> = t.type_params.iter().map(|p| l.name(p.name)).collect();
            let target = match &t.kind {
                ast::TypedefKind::Alias(a) => summarize(ast, l.names, *a, &parameters),
                ast::TypedefKind::Legacy { return_type, .. } => Summary::Function(
                    return_type.map(|r| Box::new(summarize(ast, l.names, r, &parameters))),
                ),
            };
            self.types.insert(name.clone(), Some(Alias { target }));
            let start = l.annotated_start(d.span.start, &d.metadata);
            self.push(l.declaration(
                name,
                Kind::Typedef,
                None,
                Signature::None,
                start,
                d.span.end,
            ));
        }
        for did in variables {
            let d = ast.decl(did);
            let DeclKind::Variables(list) = &d.kind else {
                continue;
            };
            let start = l.annotated_start(d.span.start, &d.metadata);
            for (i, v) in list.variables.iter().enumerate() {
                let name = l.name(v.name);
                self.union.add(&name);
                let start_ = if i == 0 { start } else { v.name.span.start };
                self.push(l.declaration(
                    name,
                    Kind::Variable,
                    None,
                    Signature::None,
                    start_,
                    l.variable_end(v),
                ));
            }
        }
    }
}

/// The parts declared by a unit, resolved from the file.
fn parts(unit: &ast::CompilationUnit, path: &Path) -> Vec<PathBuf> {
    let mut output = Vec::new();
    for d in &unit.directives {
        if let DirectiveKind::Part { uri } = &d.kind
            && let Some(relative) = dartforge_elements::load::string_lit_value(uri)
            && !relative.contains(':')
            && let Some(folder) = path.parent()
        {
            output.push(dartforge_elements::generated::key(&folder.join(relative)));
        }
    }
    output
}

fn is_part(unit: &ast::CompilationUnit) -> bool {
    unit.directives
        .iter()
        .any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))
}

/// Indexes the library at `path`: `None` when it is a part or cannot be read.
fn index(
    path: &Path,
    read_: &Reader<'_>,
    uri_of: &dyn Fn(&Path) -> String,
) -> (Vec<(PathBuf, Fingerprint)>, Option<Library>) {
    index_with_parser(path, read_, uri_of, parse_symbol_unit)
}

/// Symbol metadata needs signatures and full code ranges, not block statements.
/// Initializers, arrow bodies and unmatched-block recovery stay with the parser.
fn parse_symbol_unit(source: &str, names: &mut Interner) -> dartforge_frontend::parser::Parsed {
    dartforge_frontend::parser::parse_lexed_outline(
        source,
        dartforge_frontend::lexer::lex(source),
        names,
        dartforge_frontend::LibraryFeatures::current(),
    )
}

fn index_with_parser(
    path: &Path,
    read_: &Reader<'_>,
    uri_of: &dyn Fn(&Path) -> String,
    parse: impl Fn(&str, &mut Interner) -> dartforge_frontend::parser::Parsed,
) -> (Vec<(PathBuf, Fingerprint)>, Option<Library>) {
    let mut files = Vec::new();
    let mut union = NameUnion::empty();
    let mut types = LocalTypes::new();
    let mut units = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    // The units in pre-order: the defining one, each part and its parts.
    let mut pending = vec![path.to_path_buf()];
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut first = true;
    while let Some(file) = pending.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let Some((source, fingerprint)) = read_(&file) else {
            // A generated part can appear without changing its owner's text.
            // Its absence must participate in the owner's cache validation.
            files.push((file, Fingerprint::Missing));
            if first {
                return (files, None);
            }
            continue;
        };
        files.push((file.clone(), fingerprint));
        let mut names = Interner::new();
        let analyzed = parse(&source, &mut names);
        if first && is_part(&analyzed.unit) {
            return (files, None);
        }
        first = false;
        let read = Read {
            source: &source,
            ast: &analyzed.ast,
            names: &names,
            lines: crate::utf16::LineTable::build(&source),
            comments: Comments::scan(&source),
        };
        let mut collector = Collector {
            l: &read,
            output: Vec::new(),
            union: &mut union,
            types: &mut types,
        };
        collector.unit(&analyzed.unit);
        let declarations = collector.output;
        let uri = uri_of(&file);
        units.push(Unit { uri, declarations });
        let mut unit_path = None;
        for d in &analyzed.unit.directives {
            let (uri, configurations, output) = match &d.kind {
                DirectiveKind::Import {
                    uri,
                    configurations,
                    ..
                } => (uri, configurations, &mut imports),
                DirectiveKind::Export {
                    uri,
                    configurations,
                    ..
                } => (uri, configurations, &mut exports),
                _ => continue,
            };
            output.push(Dependency {
                unit: unit_path
                    .get_or_insert_with(|| Arc::new(file.clone()))
                    .clone(),
                primary: dartforge_elements::load::string_lit_value(uri),
                alternatives: configurations
                    .iter()
                    .filter_map(|c| dartforge_elements::load::string_lit_value(&c.uri))
                    .collect(),
            });
        }
        for p in parts(&analyzed.unit, &file).into_iter().rev() {
            pending.push(p);
        }
    }
    (
        files,
        Some(Library {
            union,
            units,
            types,
            imports,
            exports,
        }),
    )
}

impl Index {
    /// The library at `path`, from the cache when none of its files changed.
    fn library(&mut self, path: &Path, environment: &DiscoveryContext<'_>) -> Option<Arc<Library>> {
        // With the disk trusted (the watcher reported nothing since a recent
        // comparison), a file that is not open is as it was indexed.
        let current = |p: &Path, i: &Fingerprint| match environment
            .open
            .get(dartforge_elements::generated::key_of(p).as_ref())
        {
            Some(o) => o.clone(),
            None if environment.trust_disk && !matches!(i, Fingerprint::Open(..)) => i.clone(),
            None => environment
                .disk
                .state(p)
                .fingerprint
                .unwrap_or(Fingerprint::Missing),
        };
        if let Some(e) = self.entries.get_mut(path)
            && (e.checked_at == self.search_generation
                || e.files.iter().all(|(p, i)| current(p, i) == *i))
        {
            e.checked_at = self.search_generation;
            return e.library.clone();
        }
        let (files, library) = index(path, environment.read, environment.uri_of);
        let library = library.map(Arc::new);
        self.entries.insert(
            path.to_path_buf(),
            Entry {
                files,
                library: library.clone(),
                checked_at: self.search_generation,
            },
        );
        library
    }

    /// The SDK files discovered before and during `discoverAvailableFiles`:
    /// dart:core and its direct references first, then the `libraries` map
    /// of `_internal/sdk_library_metadata/lib/libraries.dart` in written order.
    fn sdk_files(&mut self, lib: &Path) -> Arc<SdkFiles> {
        if let Some(files) = &self.sdk
            && files.root == lib
        {
            return files.clone();
        }
        let layout = dartforge_elements::sdk::SdkLayout::load_as_analyzer(lib).unwrap_or_default();
        let mut list = Vec::new();
        if let Some(core) = layout.library("core") {
            list.push(dartforge_elements::generated::key(&core.path));
            if let Ok(source) = std::fs::read_to_string(&core.path) {
                let mut names = Interner::new();
                let parsed = dartforge_frontend::parser::parse(&source, &mut names);
                // AnalysisDriver._discoverDartCore calls discoverReferencedFiles:
                // exports are created before imports, regardless of source order.
                for exports in [true, false] {
                    for directive in &parsed.unit.directives {
                        let (uri, configurations) = match &directive.kind {
                            DirectiveKind::Export {
                                uri,
                                configurations,
                                ..
                            } if exports => (uri, configurations),
                            DirectiveKind::Import {
                                uri,
                                configurations,
                                ..
                            } if !exports => (uri, configurations),
                            _ => continue,
                        };
                        for uri in std::iter::once(uri).chain(configurations.iter().map(|c| &c.uri))
                        {
                            if let Some(uri) = dartforge_elements::load::string_lit_value(uri) {
                                let target = if let Some(name) = uri.strip_prefix("dart:") {
                                    layout.library(name).map(|l| l.path.clone())
                                } else {
                                    url::Url::from_file_path(&core.path)
                                        .ok()
                                        .and_then(|u| u.join(&uri).ok())
                                        .and_then(|u| u.to_file_path().ok())
                                };
                                if let Some(path) = target {
                                    list.push(dartforge_elements::generated::key(&path));
                                }
                            }
                        }
                    }
                }
            }
        }
        let available = read_libraries_dart(lib, &layout);
        let files = Arc::new(SdkFiles {
            root: lib.to_path_buf(),
            layout,
            bootstrap: list,
            available,
        });
        self.sdk = Some(files.clone());
        files
    }

    /// `FindDeclarations.compute` + the handler's `_asSymbolInformation`.
    /// `added` are the project files; `priority`, the current documents in
    /// opening order; `contexts`, each driver's package catalog and generated
    /// workspace; `open_texts`, the URI and text of the open
    /// documents by normalized path (`generated::key`); `kinds`, the
    /// `SymbolKind`s the client accepts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search(
        &mut self,
        query: &str,
        added: &[PathBuf],
        priority: &[PathBuf],
        sdk: Option<&Path>,
        contexts: &[Context],
        open_texts: &HashMap<PathBuf, (String, String)>,
        kinds: Option<&[u64]>,
        trust_disk: bool,
    ) -> Vec<Value> {
        // Each request checks the disk anew when it cannot be trusted. The
        // marker only avoids repeated checks within that same request.
        self.search_generation = match self.search_generation.checked_add(1) {
            Some(next) => next,
            None => {
                for entry in self.entries.values_mut() {
                    entry.checked_at = 0;
                }
                1
            }
        };
        // Open documents count through the editor's text (the overlay), with the
        // client's URI.
        let open: HashMap<PathBuf, Fingerprint> = open_texts
            .iter()
            .map(|(k, (_, t))| (k.clone(), text_fingerprint(t)))
            .collect();
        let reuse_untrusted = !trust_disk
            && self.paths.as_ref().is_some_and(|(last, _)| {
                last.added == added && last.sdk.as_deref() == sdk && last.contexts == contexts
            });
        let disk = DiskSnapshot::new(reuse_untrusted);
        let read = |p: &Path| -> Option<(String, Fingerprint)> {
            let k = dartforge_elements::generated::key_of(p);
            if let (Some((_, t)), Some(i)) = (open_texts.get(k.as_ref()), open.get(k.as_ref())) {
                return Some((t.clone(), i.clone()));
            }
            let i = disk.state(p).fingerprint?;
            let t = std::fs::read_to_string(p).ok()?;
            Some((t, i))
        };
        let read: &Reader<'_> = &read;
        let open = &open;
        let uri_of = |p: &Path| -> String {
            match open_texts.get(dartforge_elements::generated::key_of(p).as_ref()) {
                Some((uri, _)) => uri.clone(),
                None => url::Url::from_file_path(p)
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
            }
        };
        let sdk_files = sdk.map(|lib| self.sdk_files(lib));
        let environment = DiscoveryContext {
            catalogs: contexts,
            sdk: sdk_files.as_deref(),
            read,
            open,
            uri_of: &uri_of,
            trust_disk,
            disk: &disk,
            relative_bases: Default::default(),
            resolved: Default::default(),
        };
        // The files in `OwnedFiles` order (added ones, then the known ones that
        // were not added), kept while the disk is trusted and the inputs are the
        // same.
        let open_dependencies = priority
            .iter()
            .map(|p| {
                let library = self.library(p, &environment);
                OpenDependencies {
                    path: p.clone(),
                    imports: library
                        .as_ref()
                        .map(|l| l.imports.clone())
                        .unwrap_or_default(),
                    exports: library
                        .as_ref()
                        .map(|l| l.exports.clone())
                        .unwrap_or_default(),
                }
            })
            .collect();
        let inputs = DiscoveryInputs {
            added: added.to_vec(),
            sdk: sdk.map(Path::to_path_buf),
            contexts: contexts.to_vec(),
            priority: priority.to_vec(),
            open_dependencies,
        };
        let empty_context = Context::default();
        let has_uri = |path: &Path, context: usize| {
            contexts
                .get(context)
                .unwrap_or(&empty_context)
                .has_uri(path)
        };
        if trust_disk && self.paths.as_ref().is_some_and(|(last, _)| *last == inputs) {
            let (_, discovery) = self.paths.take().unwrap();
            return self.query_snapshot(query, inputs, discovery, kinds, &environment);
        }
        // Each driver's FileState is created once. A later encounter in that
        // driver must not steal a URI back from another driver.
        let mut discovery = match self.paths.take() {
            Some((last, mut previous))
                if reuse_untrusted
                    || (trust_disk
                        && last.added == inputs.added
                        && last.sdk == inputs.sdk
                        && last.contexts == inputs.contexts) =>
            {
                if !trust_disk {
                    previous.retain_live(open, contexts, &disk);
                }
                previous
            }
            _ => {
                let mut discovery = Discovery::default();
                for p in added {
                    let context = context_for(p, contexts);
                    if has_uri(p, context) {
                        discovery.add(p, context, contexts, true);
                    }
                }
                discovery
            }
        };
        discovery.snapshot_resolved = false;
        for context in 0..contexts.len().max(1) {
            if let Some(sdk) = &sdk_files {
                for p in &sdk.bootstrap {
                    discovery.add(p, context, contexts, false);
                }
            }
        }
        // LibraryNode.computeDependencies materializes imports (including
        // implicit dart:core) before exports, then walks those dependencies.
        // The dart:core bootstrap above uses a different discovery entry point.
        let mut pending: Vec<_> = added
            .iter()
            .rev()
            .filter_map(|p| {
                let context = context_for(p, contexts);
                discovery
                    .created
                    .contains(&(context, dartforge_elements::generated::key(p)))
                    .then(|| (context, p.clone()))
            })
            .collect();
        pending.extend(priority.iter().rev().filter_map(|p| {
            let context = context_for(p, contexts);
            (discovery
                .created
                .contains(&(context, dartforge_elements::generated::key(p)))
                || has_uri(p, context))
            .then(|| (context, p.clone()))
        }));
        self.discover_graph(&mut discovery, pending, &environment);
        let empty = Context::default();
        let mut available_contexts = context_order::order(contexts);
        if available_contexts.is_empty() {
            available_contexts.push(0);
        }
        for context in available_contexts {
            if let Some(sdk) = &sdk_files {
                for p in &sdk.available {
                    discovery.add(p, context, contexts, false);
                }
            }
            let catalog = contexts.get(context).unwrap_or(&empty);
            for (_, folder) in &catalog.packages {
                let files = match self.listed.get(folder) {
                    Some(files) if trust_disk => files.clone(),
                    _ => {
                        let mut files = Vec::new();
                        discover(folder, &mut files);
                        self.listed.insert(folder.clone(), files.clone());
                        files
                    }
                };
                for p in files {
                    discovery.add(&p, context, contexts, false);
                }
            }
        }
        // Catalog replacement and deleted files must release old declaration
        // data, rather than retaining every package version seen in a session.
        let retained: HashSet<_> = discovery
            .created
            .iter()
            .map(|(_, p)| p)
            .chain(priority)
            .map(|p| dartforge_elements::generated::key(p))
            .collect();
        self.entries.retain(|p, _| retained.contains(p));
        self.listed.retain(|folder, _| {
            contexts
                .iter()
                .flat_map(|c| &c.packages)
                .any(|(_, p)| p == folder)
        });
        self.query_snapshot(query, inputs, discovery, kinds, &environment)
    }

    /// FindDeclarations snapshots OwnedFiles before getLibraryByUri resolves
    /// each entry. Files discovered by that resolution join the next query,
    /// even when the current query's name filter matches no declarations.
    fn query_snapshot(
        &mut self,
        query: &str,
        inputs: DiscoveryInputs,
        mut discovery: Discovery,
        kinds: Option<&[u64]>,
        environment: &DiscoveryContext<'_>,
    ) -> Vec<Value> {
        let files = discovery.files.clone();
        if !discovery.snapshot_resolved {
            let created = discovery.created.len();
            let pending = files
                .iter()
                .rev()
                .map(|f| (f.context, f.path.clone()))
                .collect();
            self.discover_graph(&mut discovery, pending, environment);
            discovery.snapshot_resolved = discovery.created.len() == created;
        }
        self.paths = Some((inputs, discovery));
        self.find(query, &files, kinds, environment)
    }

    /// Materialize all directive URIs, but recurse only through the primary
    /// selected in the analysis server's empty declared-variable environment.
    /// Imports and exports have separate lazy lists and discovery order.
    fn discover_graph(
        &mut self,
        discovery: &mut Discovery,
        mut pending: Vec<(usize, PathBuf)>,
        environment: &DiscoveryContext<'_>,
    ) {
        let mut visited = environment.resolved.borrow_mut();
        let empty = Context::default();
        while let Some((context, path)) = pending.pop() {
            let path = dartforge_elements::generated::key(&path);
            if !visited.insert((context, path.clone())) {
                continue;
            }
            let Some(library) = self.library(&path, environment) else {
                continue;
            };
            let catalog = environment.catalogs.get(context).unwrap_or(&empty);
            let mut next = Vec::new();
            for (exports, dependencies) in [(false, &library.imports), (true, &library.exports)] {
                // The synthetic core import comes after explicit imports,
                // before the export list is materialized.
                if exports
                    && let Some(core) = environment.sdk.and_then(|s| s.layout.library("core"))
                {
                    next.push(core.path.clone());
                }
                for dependency in dependencies {
                    for (i, uri) in dependency
                        .primary
                        .iter()
                        .chain(&dependency.alternatives)
                        .enumerate()
                    {
                        if let Some(target) = environment.relative_bases.target(
                            context,
                            &dependency.unit,
                            uri,
                            environment.sdk,
                            catalog,
                        ) {
                            discovery.add(&target, context, environment.catalogs, false);
                            if i == 0 && dependency.primary.is_some() {
                                next.push(target);
                            }
                        }
                    }
                }
            }
            pending.extend(next.into_iter().rev().map(|p| (context, p)));
        }
    }

    /// The declarations of the owned libraries that match `query`.
    fn find(
        &mut self,
        query: &str,
        files: &[OwnedFile],
        kinds: Option<&[u64]>,
        environment: &DiscoveryContext<'_>,
    ) -> Vec<Value> {
        let libraries: Vec<Arc<Library>> = files
            .iter()
            .filter_map(|c| self.library(&c.path, environment))
            .collect();
        // The `typedef`s of every library, to expand aliases from another library
        // (the first one of each name).
        let mut global: HashMap<String, Alias> = HashMap::new();
        for b in &libraries {
            for (name, a) in &b.types {
                if let Some(a) = a {
                    global.entry(name.clone()).or_insert_with(|| a.clone());
                }
            }
        }
        let mut matcher = Matcher::new(query, Style::Text);
        let mut output = Vec::new();
        for b in &libraries {
            if !b.union.contains(query) {
                continue;
            }
            for u in &b.units {
                for d in &u.declarations {
                    if output.len() >= LIMIT {
                        return output;
                    }
                    if matcher.score(&d.name) < 0.0 {
                        continue;
                    }
                    output.push(symbol(d, &u.uri, &b.types, &global, kinds));
                }
            }
        }
        output
    }
}

/// The `SymbolInformation` of a declaration.
fn symbol(
    d: &Declaration,
    uri: &str,
    locals: &LocalTypes,
    global: &HashMap<String, Alias>,
    kinds: Option<&[u64]>,
) -> Value {
    let by_return = |return_: &Option<Summary>, augment: bool| {
        let p = return_
            .as_ref()
            .map_or(Paren::None, |r| paren(r, &[], locals, global, 0));
        // `augment ` before the return: the `(` is never at the start.
        if augment && p == Paren::StartsWith {
            Paren::Contains
        } else {
            p
        }
    };
    // `parameters`: the displayed text from the first `(` (which cannot be the
    // first character); `()` when it is only the empty list.
    let suffix = match &d.signature {
        Signature::None => "",
        Signature::List { empty } => {
            if *empty {
                "()"
            } else {
                "(…)"
            }
        }
        Signature::Getter { return_, augment } => match by_return(return_, *augment) {
            Paren::Contains => "(…)",
            _ => "",
        },
        Signature::Executable {
            return_,
            empty,
            augment,
        } => match by_return(return_, *augment) {
            Paren::StartsWith => "",
            Paren::Contains => "(…)",
            Paren::None if *empty => "()",
            Paren::None => "(…)",
        },
    };
    let accepts = |k: u64| kinds.map_or(k <= 18, |l| l.contains(&k));
    let kind = d
        .kind
        .preferences()
        .iter()
        .copied()
        .find(|k| accepts(*k))
        .unwrap_or(19);
    let mut v = json!({
        "name": format!("{}{suffix}", d.name),
        "kind": kind,
        "location": {
            "uri": uri,
            "range": {
                "start": {"line": d.start.0, "character": d.start.1},
                "end": {"line": d.end.0, "character": d.end.1},
            },
        },
    });
    if let Some(c) = &d.container {
        v["containerName"] = json!(c);
    }
    v
}

/// `discoverRecursively`: the `.dart` files of the folder and its subfolders,
/// in file system order (the same as Dart's `Directory.listSync`).
fn discover(folder: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let Ok(ty) = e.file_type() else { continue };
        if ty.is_file() {
            if path.extension().is_some_and(|x| x == "dart") {
                output.push(path);
            }
        } else if ty.is_dir() {
            discover(&path, output);
        }
    }
}

/// The entries of the `libraries` map of the SDK's `libraries.dart`, in
/// written order: `'name': const LibraryInfo('path', …)`.
fn read_libraries_dart(lib: &Path, layout: &dartforge_elements::sdk::SdkLayout) -> Vec<PathBuf> {
    let file = lib
        .join("_internal")
        .join("sdk_library_metadata")
        .join("lib")
        .join("libraries.dart");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    let Some(start) = text.find("libraries = const {") else {
        return Vec::new();
    };
    let body = &text[start..];
    let end = body.find("\n};").unwrap_or(body.len());
    let body = &body[..end];
    let mut output = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("LibraryInfo(") {
        // Preserve metadata insertion order, but resolve the name through
        // the analyzer layout. Its dart2jsPath may replace the positional
        // path; indexing both would duplicate their shared part declarations.
        let before = rest[..i].trim_end();
        let before = before.strip_suffix("const").unwrap_or(before).trim_end();
        let name = before.strip_suffix(':').and_then(|before| {
            let before = before.trim_end();
            let quote = before.chars().last().filter(|c| matches!(c, '\'' | '"'))?;
            let before = &before[..before.len() - 1];
            let start = before.rfind(quote)?;
            Some(&before[start + 1..])
        });
        if let Some(library) = name.and_then(|n| layout.library(n)) {
            output.push(dartforge_elements::generated::key(&library.path));
        }
        rest = &rest[i + "LibraryInfo(".len()..];
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(packages: &[(String, PathBuf)]) -> [Context; 1] {
        [Context {
            packages: packages.to_vec(),
            ..Context::default()
        }]
    }

    pub(super) fn fixture(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dartforge-symbol-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn symbol_names(symbols: Vec<Value>) -> Vec<String> {
        symbols
            .iter()
            .filter_map(|s| s["name"].as_str().map(str::to_owned))
            .collect()
    }

    #[test]
    fn disk_metadata_is_shared_only_within_the_current_request() {
        let root = fixture("disk-snapshot");
        let file = root.join("entry.dart");
        let absent = DiskSnapshot::new(true);
        assert!(!absent.state(&file).is_file);
        assert_eq!(absent.state(&file).fingerprint, None);
        std::fs::write(&file, "a").unwrap();
        assert!(!absent.state(&file).is_file);
        let present = DiskSnapshot::new(true);
        let previous = present.state(&file).fingerprint;
        assert!(present.state(&file).is_file);
        std::fs::write(&file, "a longer declaration").unwrap();
        assert_eq!(present.state(&file).fingerprint, previous);
        let edited = DiskSnapshot::new(true);
        assert_ne!(edited.state(&file).fingerprint, previous);
        assert!(edited.state(&file).is_file);
        std::fs::remove_file(&file).unwrap();
        assert!(edited.state(&file).is_file);
        assert!(!DiskSnapshot::new(true).state(&file).is_file);
        let uncached = DiskSnapshot::new(false);
        assert!(!uncached.state(&file).is_file);
        std::fs::write(&file, "visible without sharing").unwrap();
        assert!(uncached.state(&file).is_file);
        std::fs::remove_file(&file).unwrap();
        let directory = DiskSnapshot::new(true);
        assert!(!directory.state(&root).is_file);
        assert!(directory.state(&root).fingerprint.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shared_container_names_follow_library_replacement() {
        let root = fixture("container-names");
        let path = root.join("entry.dart");
        let source = "class ContainerAlpha { ContainerAlpha.memberCtor(); int memberField = 0; int get memberGetter => 0; set memberSetter(int value) {} void memberMethod() {} } class ContainerBeta { int memberField = 1; } extension type ContainerWrapper(int memberRepresentation) { void memberMethod() {} }";
        std::fs::write(&path, source).unwrap();
        let added = [path.clone()];
        let contexts = [Context::default()];
        let mut open = HashMap::new();
        let mut index = Index::default();
        let before = index.search("member", &added, &[], None, &contexts, &open, None, false);
        let previous = {
            let library = index.entries[&path].library.as_ref().unwrap();
            let declarations = &library.units[0].declarations;
            for name in ["ContainerAlpha", "ContainerWrapper"] {
                let names: Vec<_> = declarations
                    .iter()
                    .filter_map(|d| d.container.as_ref())
                    .filter(|c| c.as_ref() == name)
                    .collect();
                assert!(names.len() > 1);
                assert!(names.iter().all(|c| Arc::ptr_eq(c, names[0])));
            }
            let name = declarations
                .iter()
                .filter_map(|d| d.container.as_ref())
                .find(|c| c.as_ref() == "ContainerAlpha")
                .unwrap();
            Arc::downgrade(name)
        };
        let uri = url::Url::from_file_path(&path).unwrap().to_string();
        open.insert(
            path.clone(),
            (uri, source.replace("ContainerAlpha", "ContainerRenamed")),
        );
        let after = index.search("member", &added, &added, None, &contexts, &open, None, true);
        assert!(
            after
                .iter()
                .any(|s| s["containerName"] == "ContainerRenamed")
        );
        assert!(!after.iter().any(|s| s["containerName"] == "ContainerAlpha"));
        assert!(
            before
                .iter()
                .any(|s| s["containerName"] == "ContainerAlpha")
        );
        assert!(
            previous.upgrade().is_none(),
            "the replaced container remained retained"
        );
        assert_eq!(
            after,
            Index::default().search("member", &added, &added, None, &contexts, &open, None, true)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn symbol_outlines_preserve_full_parser_metadata() {
        let root = fixture("outline-metadata");
        let sources = [
            "/// 😀 Documentation\n@deprecated class Probe<T> {\n".to_owned()
                + "  int field = (() { return 1; })();\n"
                + "  Probe.named() : field = 2 { print(field); }\n"
                + "  int get value { return field; }\n"
                + "  set value(int n) { field = n; }\n"
                + "  T method(T value) { void local() {} return value; }\n}\n",
            "typedef Callback<T> = T Function(int);\n".to_owned()
                + "Callback<int> callback() { return (n) => n; }\n"
                + "var literal = (int n) { return n + 1; };\n"
                + "var arrow = (int n) => n;\n"
                + "(int, String) record() { return (1, 'x'); }\n",
            "enum Probe { one, two; int method() { return index; } }\n".to_owned()
                + "mixin Mix { void method() {} }\n"
                + "extension Ext on int { int get next => this + 1; }\n"
                + "extension type Wrapper(int value) { void method() {} }\n",
            "void broken() { var x = ; } class After {}".into(),
            "void broken() { class Local {} } class After {}".into(),
            "class Open { void broken() { var x = 1;".into(),
            "void broken() { print('unterminated); } class After {}".into(),
        ];
        let uri_of = |p: &Path| url::Url::from_file_path(p).unwrap().to_string();
        for (i, source) in sources.iter().enumerate() {
            let file = root.join(format!("case_{i}.dart"));
            let read = |p: &Path| (p == file).then(|| (source.clone(), text_fingerprint(source)));
            let full = index_with_parser(&file, &read, &uri_of, dartforge_frontend::parser::parse);
            let outline = index_with_parser(&file, &read, &uri_of, parse_symbol_unit);
            assert_eq!(outline, full, "source {i}: {source}");
        }
        // Optional real-world expansion of the same metadata invariant. The
        // fixed sources above always run; no SDK or corpus is required in CI.
        if let Some(manifests) = std::env::var_os("DARTFORGE_SYMBOL_CORPUS_MANIFESTS") {
            let mut files = std::collections::BTreeSet::new();
            for manifest in std::env::split_paths(&manifests) {
                let data: Value =
                    serde_json::from_str(&std::fs::read_to_string(manifest).unwrap()).unwrap();
                for file in data["files"].as_array().unwrap() {
                    let path = PathBuf::from(file["path"].as_str().unwrap());
                    if path.extension().is_some_and(|e| e == "dart") {
                        files.insert(path);
                    }
                }
            }
            assert!(
                !files.is_empty(),
                "the supplied corpus must contain Dart files"
            );
            let read = |p: &Path| {
                std::fs::read_to_string(p).ok().map(|s| {
                    let fingerprint = text_fingerprint(&s);
                    (s, fingerprint)
                })
            };
            let count = files.len();
            for file in files {
                let full =
                    index_with_parser(&file, &read, &uri_of, dartforge_frontend::parser::parse);
                let outline = index_with_parser(&file, &read, &uri_of, parse_symbol_unit);
                assert_eq!(outline, full, "corpus file: {}", file.display());
            }
            eprintln!("Compared complete symbol metadata for {count} corpus files");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_missing_part_can_appear_without_editing_its_owner() {
        let root = fixture("part-created");
        let owner = root.join("owner.dart");
        let part = root.join("generated.dart");
        std::fs::write(&owner, "part 'generated.dart'; class PartProbeOwner {}").unwrap();
        let contexts = [Context::default()];
        let open = HashMap::new();
        let mut index = Index::default();
        assert_eq!(
            symbol_names(index.search(
                "PartProbe",
                std::slice::from_ref(&owner),
                &[],
                None,
                &contexts,
                &open,
                None,
                false
            )),
            ["PartProbeOwner"]
        );
        std::fs::write(&part, "part of 'owner.dart'; class PartProbeCreated {}").unwrap();
        let added = [owner, part];
        let warm = index.search(
            "PartProbe",
            &added,
            &[],
            None,
            &contexts,
            &open,
            None,
            false,
        );
        let fresh = Index::default().search(
            "PartProbe",
            &added,
            &[],
            None,
            &contexts,
            &open,
            None,
            false,
        );
        assert_eq!(
            symbol_names(fresh.clone()),
            ["PartProbeOwner", "PartProbeCreated"]
        );
        assert_eq!(warm, fresh);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validation_generation_wrap_still_observes_disk_edits() {
        let root = fixture("validation-generation");
        let file = root.join("entry.dart");
        std::fs::write(&file, "class GenerationProbeOne {}").unwrap();
        let contexts = [Context::default()];
        let added = [file.clone()];
        let open = HashMap::new();
        let mut index = Index::default();
        let search = |index: &mut Index| {
            index.search(
                "GenerationProbe",
                &added,
                &[],
                None,
                &contexts,
                &open,
                None,
                false,
            )
        };
        assert_eq!(symbol_names(search(&mut index)), ["GenerationProbeOne"]);
        index.search_generation = u64::MAX;
        std::fs::write(&file, "class GenerationProbeReplacement {}").unwrap();
        let result = search(&mut index);
        assert_eq!(symbol_names(result.clone()), ["GenerationProbeReplacement"]);
        assert_eq!(result, search(&mut Index::default()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shadowed_nonlib_source_is_not_added_but_its_built_import_is_known() {
        let root = fixture("uri-added-shadow");
        let lib = root.join("lib");
        let output = root.join("generated");
        for dir in [
            &lib,
            &root.join("test"),
            &output.join("lib"),
            &output.join("test"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let entry = lib.join("entry.dart");
        let shadow = root.join("test/shadow.dart");
        std::fs::write(&entry, "class ProbeEntry {}").unwrap();
        std::fs::write(
            &shadow,
            "import '../foreign.dart'; class ProbeSourceShadow {}",
        )
        .unwrap();
        std::fs::write(root.join("foreign.dart"), "class ProbeForeign {}").unwrap();
        std::fs::write(
            output.join("lib/entry.dart"),
            "class ProbeWrongBuiltEntry {}",
        )
        .unwrap();
        std::fs::write(output.join("test/shadow.dart"), "class ProbeBuiltShadow {}").unwrap();
        let contexts = [Context {
            root: root.clone(),
            packages: vec![("p".into(), lib.clone())],
            generated: vec![("p".into(), lib, output.clone())],
            generated_files: vec![(root.clone(), output.clone())],
            ..Context::default()
        }];
        let mut index = Index::default();
        let added = [entry.clone(), shadow.clone()];
        let mut open = HashMap::new();
        assert_eq!(
            symbol_names(index.search("Probe", &added, &[], None, &contexts, &open, None, false)),
            ["ProbeEntry"]
        );
        let entry_uri = url::Url::from_file_path(&entry).unwrap().to_string();
        let shadow_uri = url::Url::from_file_path(&shadow).unwrap();
        open.insert(
            entry.clone(),
            (
                entry_uri,
                format!("import '{shadow_uri}'; class ProbeEntry {{}}"),
            ),
        );
        let result = index.search(
            "Probe",
            &added,
            &[entry],
            None,
            &contexts,
            &open,
            None,
            true,
        );
        assert_eq!(
            symbol_names(result.clone()),
            ["ProbeEntry", "ProbeBuiltShadow"]
        );
        assert_eq!(
            result[1]["location"]["uri"],
            url::Url::from_file_path(output.join("test/shadow.dart"))
                .unwrap()
                .as_str()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn conditional_uris_and_query_discovery_keep_separate_snapshots() {
        let root = fixture("conditional-snapshots");
        for (file, source) in [
            (
                "entry.dart",
                "import 'base.dart' if (dart.library.io) 'io.dart' if (dart.library.io == 'false') 'false.dart' if (custom.setting == '') 'empty.dart'; export 'export.dart' if (dart.library.html) 'html.dart'; class ProbeEntry {}",
            ),
            ("base.dart", "import 'base_leaf.dart'; class ProbeBase {}"),
            ("io.dart", "import 'io_leaf.dart'; class ProbeIo {}"),
            ("false.dart", "class ProbeFalse {}"),
            ("empty.dart", "class ProbeEmpty {}"),
            (
                "export.dart",
                "import 'export_leaf.dart'; class ProbeExport {}",
            ),
            ("html.dart", "import 'html_leaf.dart'; class ProbeHtml {}"),
            ("base_leaf.dart", "class ProbeBaseLeaf {}"),
            ("io_leaf.dart", "class ProbeIoLeaf {}"),
            ("export_leaf.dart", "class ProbeExportLeaf {}"),
            ("html_leaf.dart", "class ProbeHtmlLeaf {}"),
        ] {
            std::fs::write(root.join(file), source).unwrap();
        }
        let added = [root.join("entry.dart")];
        let contexts = [Context {
            root: root.clone(),
            ..Context::default()
        }];
        let open = HashMap::new();
        let mut index = Index::default();
        let first =
            symbol_names(index.search("Probe", &added, &[], None, &contexts, &open, None, false));
        assert_eq!(
            first,
            [
                "ProbeEntry",
                "ProbeBase",
                "ProbeIo",
                "ProbeFalse",
                "ProbeEmpty",
                "ProbeExport",
                "ProbeHtml",
                "ProbeBaseLeaf",
                "ProbeExportLeaf"
            ]
        );
        assert!(!index.paths.as_ref().unwrap().1.snapshot_resolved);
        let second = index.search("Probe", &added, &[], None, &contexts, &open, None, true);
        let names = symbol_names(second.clone());
        assert_eq!(&names[..first.len()], first);
        assert_eq!(&names[first.len()..], ["ProbeIoLeaf", "ProbeHtmlLeaf"]);
        assert!(index.paths.as_ref().unwrap().1.snapshot_resolved);
        assert_eq!(
            index.search("Probe", &added, &[], None, &contexts, &open, None, true),
            second
        );
        // A later request must materialize these directives again. The edited
        // nonselected branch is resolved after the response snapshot, so its
        // new dependency participates only in the following response.
        std::fs::write(
            root.join("io.dart"),
            "import 'late.dart'; class ProbeIoChanged {}",
        )
        .unwrap();
        std::fs::write(root.join("late.dart"), "class ProbeLate {}").unwrap();
        let changed =
            symbol_names(index.search("Probe", &added, &[], None, &contexts, &open, None, false));
        assert!(changed.iter().any(|n| n == "ProbeIoChanged"));
        assert!(!changed.iter().any(|n| n == "ProbeLate"));
        let following =
            symbol_names(index.search("Probe", &added, &[], None, &contexts, &open, None, true));
        assert_eq!(&following[..changed.len()], changed);
        assert_eq!(&following[changed.len()..], ["ProbeLate"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unmatched_query_still_resolves_available_libraries() {
        let root = fixture("query-resolution");
        let package = root.join("package");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("available.dart"),
            format!(
                "import '{}'; class ProbeAvailable {{}}",
                url::Url::from_file_path(root.join("outside.dart")).unwrap()
            ),
        )
        .unwrap();
        std::fs::write(root.join("outside.dart"), "class ProbeOutside {}").unwrap();
        let contexts = catalog(&[("dependency".into(), package)]);
        let open = HashMap::new();
        let mut index = Index::default();
        assert!(
            index
                .search(
                    "NoSuchSymbol",
                    &[],
                    &[],
                    None,
                    &contexts,
                    &open,
                    None,
                    false
                )
                .is_empty()
        );
        assert_eq!(
            symbol_names(index.search("Probe", &[], &[], None, &contexts, &open, None, true)),
            ["ProbeAvailable", "ProbeOutside"]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_parent_import_cannot_reclaim_a_live_generated_uri() {
        let root = fixture("missing-owner");
        let child = root.join("example");
        let package = root.join("shared/lib");
        let generated = child.join(".dart_tool/build/generated/shared/lib");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(&generated).unwrap();
        let parent_entry = root.join("parent.dart");
        let child_entry = child.join("child.dart");
        let imports = "import 'package:shared/api.template.dart';";
        std::fs::write(&parent_entry, imports).unwrap();
        std::fs::write(&child_entry, imports).unwrap();
        std::fs::write(
            generated.join("api.template.dart"),
            "class StableTemplate {}",
        )
        .unwrap();
        let contexts = [
            Context {
                root: root.clone(),
                packages: vec![("shared".into(), package.clone())],
                ..Context::default()
            },
            Context {
                root: child,
                packages: vec![("shared".into(), package.clone())],
                generated: vec![("shared".into(), package, generated.clone())],
                ..Context::default()
            },
        ];
        let mut symbols = Index::default();
        let added = [parent_entry.clone(), child_entry];
        let mut open = HashMap::new();
        let before = symbols.search(
            "StableTemplate",
            &added,
            &added[..1],
            None,
            &contexts,
            &open,
            None,
            false,
        );
        assert_eq!(before.len(), 1);
        assert_eq!(
            before[0]["location"]["uri"],
            url::Url::from_file_path(generated.join("api.template.dart"))
                .unwrap()
                .to_string()
        );
        // Revalidating disk state must preserve the child's owner, including
        // the fact that the parent's missing FileState was already created.
        assert_eq!(
            symbols.search(
                "StableTemplate",
                &added,
                &added[..1],
                None,
                &contexts,
                &open,
                None,
                false
            ),
            before
        );
        for i in 0..128 {
            let text = format!("{imports} import 'package:shared/missing_{i}.dart';");
            open.insert(
                parent_entry.clone(),
                (
                    url::Url::from_file_path(&parent_entry).unwrap().to_string(),
                    text,
                ),
            );
            assert_eq!(
                symbols.search(
                    "StableTemplate",
                    &added,
                    &added[..1],
                    None,
                    &contexts,
                    &open,
                    None,
                    false
                ),
                before
            );
        }
        let state = &symbols.paths.as_ref().unwrap().1;
        assert!(
            state.created.len() < 12,
            "missing-import history retained {} file states",
            state.created.len()
        );
        assert!(
            symbols.entries.len() < 12,
            "missing-import history retained {} indexes",
            symbols.entries.len()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn priorities_and_unsaved_imports_preserve_known_file_order() {
        let root = fixture("priority");
        let package = root.join("dependency/lib");
        std::fs::create_dir_all(&package).unwrap();
        let a = root.join("a.dart");
        let b = root.join("b.dart");
        std::fs::write(
            &a,
            "import 'package:dependency/a.dart'; class NeedleEntryA {}",
        )
        .unwrap();
        std::fs::write(
            &b,
            "import 'package:dependency/b.dart'; class NeedleEntryB {}",
        )
        .unwrap();
        std::fs::write(package.join("a.dart"), "class NeedleDependencyA {}").unwrap();
        std::fs::write(package.join("b.dart"), "class NeedleDependencyB {}").unwrap();
        let added = [b.clone(), a.clone()];
        let packages = [("dependency".to_string(), package.join("../lib"))];
        let mut index = Index::default();
        let symbols = index.search(
            "Needle",
            &added,
            std::slice::from_ref(&a),
            None,
            &catalog(&packages),
            &HashMap::new(),
            None,
            false,
        );
        assert_eq!(
            symbols[2]["location"]["uri"],
            url::Url::from_file_path(package.join("a.dart"))
                .unwrap()
                .to_string()
        );
        let before = symbol_names(symbols);
        assert_eq!(
            before,
            [
                "NeedleEntryB",
                "NeedleEntryA",
                "NeedleDependencyA",
                "NeedleDependencyB"
            ]
        );
        // Closing the first priority file does not reorder known dependencies.
        assert_eq!(
            symbol_names(index.search(
                "Needle",
                &added,
                std::slice::from_ref(&b),
                None,
                &catalog(&packages),
                &HashMap::new(),
                None,
                true,
            )),
            before
        );

        let late = root.join("late.dart");
        std::fs::write(late, "class NeedleLate {}").unwrap();
        let changed = "import 'late.dart'; class NeedleEntryB {}";
        let mut open = HashMap::from([(
            b.clone(),
            ("file:///b.dart".to_string(), changed.to_string()),
        )]);
        let expected = [
            "NeedleEntryB",
            "NeedleEntryA",
            "NeedleDependencyA",
            "NeedleDependencyB",
            "NeedleLate",
        ];
        assert_eq!(
            symbol_names(index.search(
                "Needle",
                &added,
                std::slice::from_ref(&b),
                None,
                &catalog(&packages),
                &open,
                None,
                true,
            )),
            expected
        );
        let body_edit = "import 'late.dart'; class NeedleEntryB { void f() {} }";
        open.get_mut(&b).unwrap().1 = body_edit.to_string();
        let symbols = index.search(
            "Needle",
            &added,
            std::slice::from_ref(&b),
            None,
            &catalog(&packages),
            &open,
            None,
            true,
        );
        assert_eq!(
            symbols[0]["location"]["range"]["end"]["character"],
            body_edit.len()
        );
        assert_eq!(symbol_names(symbols), expected);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacing_a_package_catalog_releases_its_old_libraries() {
        let root = fixture("catalog");
        let old = root.join("old/lib");
        let new = root.join("new/lib");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("api.dart"), "class NeedleOld {}").unwrap();
        std::fs::write(new.join("api.dart"), "class NeedleNew {}").unwrap();
        let entry = root.join("entry.dart");
        std::fs::write(
            &entry,
            "import 'package:dependency/api.dart'; class NeedleEntry {}",
        )
        .unwrap();
        let mut index = Index::default();
        assert_eq!(
            symbol_names(index.search(
                "Needle",
                std::slice::from_ref(&entry),
                &[],
                None,
                &catalog(&[("dependency".to_string(), old.clone())]),
                &HashMap::new(),
                None,
                false,
            )),
            ["NeedleEntry", "NeedleOld"]
        );
        let old_library = Arc::downgrade(
            index.entries[&old.join("api.dart")]
                .library
                .as_ref()
                .unwrap(),
        );
        assert_eq!(
            symbol_names(index.search(
                "Needle",
                &[entry],
                &[],
                None,
                &catalog(&[("dependency".to_string(), new)]),
                &HashMap::new(),
                None,
                true,
            )),
            ["NeedleEntry", "NeedleNew"]
        );
        assert!(
            old_library.upgrade().is_none(),
            "the old library is still retained"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sdk_discovery_indexes_only_the_analyzer_selected_variant() {
        let root = std::env::temp_dir().join(format!(
            "dartforge-symbol-sdk-variant-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let metadata = root.join("_internal/sdk_library_metadata/lib");
        std::fs::create_dir_all(&metadata).unwrap();
        std::fs::write(
            metadata.join("libraries.dart"),
            "final libraries = const {\n  'core': const LibraryInfo('core.dart'),\n  'platform': const LibraryInfo('default.dart', dart2jsPath: 'selected.dart'),\n};\n",
        )
        .unwrap();
        for (name, text) in [
            ("core.dart", "import 'dart:platform'; class NeedleCore {}"),
            (
                "default.dart",
                "library platform; part 'shared.dart'; class NeedleDefault {}",
            ),
            (
                "selected.dart",
                "library platform; part 'shared.dart'; class NeedleSelected {}",
            ),
            ("shared.dart", "part of platform; class NeedlePart {}"),
        ] {
            std::fs::write(root.join(name), text).unwrap();
        }
        let mut index = Index::default();
        for trust_disk in [false, true] {
            let symbols = index.search(
                "Needle",
                &[],
                &[],
                Some(&root),
                &catalog(&[]),
                &HashMap::new(),
                None,
                trust_disk,
            );
            let names: Vec<_> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
            assert_eq!(names, ["NeedleCore", "NeedleSelected", "NeedlePart"]);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sdk_bootstrap_precedes_library_discovery_and_survives_the_cache() {
        let root = std::env::temp_dir().join(format!(
            "dartforge-symbol-sdk-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let metadata = root.join("_internal/sdk_library_metadata/lib");
        std::fs::create_dir_all(&metadata).unwrap();
        std::fs::write(
            metadata.join("libraries.dart"),
            "final libraries = const {
  'async': const LibraryInfo('async.dart'),
  'other': const LibraryInfo('other.dart'),
  'collection': const LibraryInfo('collection.dart'),
  '_internal': const LibraryInfo('internal.dart'),
  'core': const LibraryInfo('core.dart'),
};\n",
        )
        .unwrap();
        std::fs::write(
            root.join("core.dart"),
            "import 'dart:collection'; import 'dart:_internal';
export 'dart:async'; class NeedleCore {}\n",
        )
        .unwrap();
        for name in ["Async", "Other", "Collection", "Internal"] {
            std::fs::write(
                root.join(format!("{}.dart", name.to_ascii_lowercase())),
                format!("class Needle{name} {{}}\n"),
            )
            .unwrap();
        }
        let mut index = Index::default();
        for trust_disk in [false, true] {
            let symbols = index.search(
                "Needle",
                &[],
                &[],
                Some(&root),
                &catalog(&[]),
                &HashMap::new(),
                None,
                trust_disk,
            );
            let names: Vec<_> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
            assert_eq!(
                names,
                [
                    "NeedleCore",
                    "NeedleAsync",
                    "NeedleCollection",
                    "NeedleInternal",
                    "NeedleOther",
                ],
                "trust_disk={trust_disk}"
            );
        }
        // A dependency reached during analysis precedes unrelated available
        // SDK libraries, even when its file is exported by another dependency.
        let package = root.join("dependency/lib");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("api.dart"),
            "export 'detail.dart'; import 'imported.dart'; class NeedleApi {}\n",
        )
        .unwrap();
        std::fs::write(package.join("detail.dart"), "class NeedleDetail {}\n").unwrap();
        std::fs::write(package.join("imported.dart"), "class NeedleImported {}\n").unwrap();
        let entry = root.join("entry.dart");
        std::fs::write(
            &entry,
            "import 'package:dependency/api.dart'; class NeedleEntry {}\n",
        )
        .unwrap();
        let mut index = Index::default();
        let symbols = index.search(
            "Needle",
            &[entry],
            &[],
            Some(&root),
            &catalog(&[("dependency".to_string(), package)]),
            &HashMap::new(),
            None,
            false,
        );
        let names: Vec<_> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
        assert_eq!(
            names,
            [
                "NeedleEntry",
                "NeedleCore",
                "NeedleAsync",
                "NeedleCollection",
                "NeedleInternal",
                "NeedleApi",
                "NeedleImported",
                "NeedleDetail",
                "NeedleOther",
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn union_like_analyzer() {
        let mut u = NameUnion::empty();
        u.add("parseArgs");
        assert!(u.contains("pa"));
        assert!(u.contains("PARSE"));
        assert!(u.contains("p_a"));
        assert!(!u.contains("xyz"));
        assert!(!u.contains("parseArgsX"));
    }

    #[test]
    fn unary_minus_uses_the_element_name_in_symbol_search() {
        let source = "class Negation { Negation operator -() => this; Negation operator -(Negation other) => this; }";
        let path = PathBuf::from("/operators.dart");
        let read = |_: &Path| Some((source.to_string(), text_fingerprint(source)));
        let (_, library) = index(&path, &read, &|_| "file:///operators.dart".to_string());
        let library = library.unwrap();
        assert!(library.union.contains("unary"));
        let operators: Vec<_> = library.units[0]
            .declarations
            .iter()
            .filter(|d| matches!(d.kind, Kind::Method))
            .map(|d| {
                symbol(
                    d,
                    "file:///operators.dart",
                    &library.types,
                    &HashMap::new(),
                    None,
                )["name"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(operators, ["unary-()", "-(…)"]);
    }

    #[test]
    fn suffixes_by_return() {
        let locals = LocalTypes::new();
        let global = HashMap::new();
        let f = Declaration {
            name: "f".into(),
            kind: Kind::Function,
            container: None,
            signature: Signature::Executable {
                return_: Some(Summary::Registration),
                empty: true,
                augment: false,
            },
            start: (0, 0),
            end: (0, 1),
        };
        assert_eq!(
            symbol(&f, "file:///a.dart", &locals, &global, None)["name"],
            "f"
        );
        let g = Declaration {
            signature: Signature::Executable {
                return_: Some(Summary::Function(None)),
                empty: true,
                augment: false,
            },
            ..f.clone()
        };
        assert_eq!(
            symbol(&g, "file:///a.dart", &locals, &global, None)["name"],
            "f(…)"
        );
        let h = Declaration {
            signature: Signature::Executable {
                return_: None,
                empty: true,
                augment: false,
            },
            ..f.clone()
        };
        assert_eq!(
            symbol(&h, "file:///a.dart", &locals, &global, None)["name"],
            "f()"
        );
        let k = Declaration {
            kind: Kind::EnumConstant,
            signature: Signature::None,
            ..f
        };
        assert_eq!(
            symbol(&k, "file:///a.dart", &locals, &global, None)["kind"],
            10
        );
    }
}
