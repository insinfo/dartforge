//! Semantic identity of declarations over a loaded program: what the cursor
//! denotes, where the declaration is and all of its occurrences.
//!
//! It is the single model behind `definition`, `hover`, `references` and
//! `rename`. What a reference denotes comes from the side tables of the
//! common inference of `crates/types` (`get_resolved`, `local_declaration`,
//! `local_types`) and from the namespaces of `crates/elements`
//! (`lookup`, `lookup_prefixed`), never from a parallel resolution by name:
//!
//! * local, parameter or local function — the declaration and the
//!   expressions that refer to it; for a named parameter, the `name:` labels
//!   of the calls and the homonymous parameter of the method's overrides;
//! * class or extension member — the family linked by overriding (walks up
//!   and down the hierarchy until it closes), getter and setter together;
//! * top-level declaration — uses, type annotations, constructors written
//!   with the class name, `show`/`hide` and metadata;
//! * named constructor, import prefix and type parameter;
//! * `[name]` references of documentation comments, resolved in the scope
//!   of the documented declaration.
//!
//! A [`Project`] is transient: it belongs to one request or to the bounded
//! session of [`crate::session`], and is discarded when any text changes.

use crate::DocumentStore;
use crate::query::Query;
use crate::dartdoc;
use crate::semantic::SemanticAnalyzer;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, LibraryId,
    Program, UnitId, VariableId, VariableRef,
};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::{self, Ast, DeclKind, ExprKind, MemberKind, Parameter};
use dartforge_intern::SymbolId;
use dartforge_types::{MemberRef, Resolved};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use url::Url;

/// Ceiling of `.dart` files scanned at the project root.
const MAX_FILES: usize = 20_000;

/// The loaded program, with the bodies inferred for the libraries where uses
/// are searched.
pub(crate) struct Project {
    pub(crate) query: Query,
    pub(crate) root: PathBuf,
    /// Libraries whose bodies were inferred (the project's, or only the
    /// document's in a library query). Uses are only searched in them.
    pub(crate) libraries: HashSet<LibraryId>,
    /// The `referencedNames` of each unit already computed
    /// (`crate::referenced_names`).
    pub(crate) referenced_names: std::sync::Mutex<std::collections::HashMap<UnitId, std::sync::Arc<HashSet<String>>>>,
    /// The reverse index name → units that cite it
    /// (`crate::referenced_names::ReverseIndex`), maintained per unit.
    pub(crate) reverse_index: std::sync::Mutex<crate::referenced_names::ReverseIndex>,
    /// The documentation comments of each unit already read (with the size of
    /// the text they came from).
    pub(crate) comments: std::sync::Mutex<std::collections::HashMap<UnitId, ReadComments>>,
    /// The occurrence index of each unit already walked
    /// ([`Project::unit_occurrences`]), with the stamp (text size and number of
    /// expressions) it came from.
    pub(crate) occurrences_by_unit: std::sync::Mutex<std::collections::HashMap<UnitId, StampedOccurrences>>,
    /// [`Project::written_subtypes`], computed once and forgotten whenever a
    /// unit's text changes ([`Project::forget_unit`]).
    pub(crate) written_subtypes: std::sync::Mutex<Option<std::sync::Arc<WrittenSubtypes>>>,
}

/// The direct subtypes of each class through the index relations.
pub(crate) type WrittenSubtypes = std::collections::HashMap<ClassId, Vec<ClassId>>;

/// The comments of a unit with the size of the text they came from.
pub(crate) type ReadComments = (usize, std::sync::Arc<Vec<dartdoc::Comment>>);

/// The occurrence index of a unit with the stamp (text size, number of
/// expressions) it came from.
pub(crate) type StampedOccurrences = ((usize, usize), std::sync::Arc<UnitOccurrences>);

/// The reverse index of a unit: for each top-level element, function and
/// member variable, the names that resolve to it in its expressions (the
/// identifier or the name of the access), in expression order.
#[derive(Debug, Default)]
pub(crate) struct UnitOccurrences {
    pub(crate) elements: std::collections::HashMap<Element, Vec<Span>>,
    pub(crate) functions: std::collections::HashMap<FunctionElementId, Vec<Span>>,
    pub(crate) variables: std::collections::HashMap<VariableId, Vec<Span>>,
    /// The names of type annotations and metadata, by top-level element.
    pub(crate) type_names: std::collections::HashMap<Element, Vec<Span>>,
}

/// What a position denotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// Local, parameter or local function: unit and offset of the declared name.
    Local { unit: UnitId, declaration: usize },
    /// Class or extension member, by base name (without the setter's `=`).
    Member {
        owner: Owner,
        name: String,
        is_static: bool,
    },
    /// Top-level declaration.
    Top(Element),
    /// Named constructor (the unnamed one denotes the class).
    Constructor(FunctionElementId),
    /// Import prefix of a library.
    Prefix {
        library: LibraryId,
        name: SymbolId,
    },
    /// Type parameter: unit and offset of the declared name.
    TypeParameter { unit: UnitId, declaration: usize },
}

/// Owner of a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    Class(ClassId),
    Extension(ExtensionId),
}

/// The concrete element behind a member or top-level [`Target`] (rename's
/// family covers several; definition and hover want this one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Concrete {
    Function(FunctionElementId),
    Variable(VariableId),
}

/// The result of [`Project::identify`].
#[derive(Debug, Clone)]
pub(crate) struct Denoted {
    pub target: Target,
    /// The range of the name under the cursor.
    pub name: Span,
    /// The reference's expression, when the cursor is on a use (its static type
    /// is the hover's, already with promotions).
    pub expr: Option<ast::ExprId>,
    /// Concrete function or variable, when there is one.
    pub concrete: Option<Concrete>,
}

/// Project root of `file`: the closest directory with `pubspec.yaml`;
/// without one, the file's own directory.
pub(crate) fn project_root(file: &Path) -> PathBuf {
    let mut current = file.parent();
    while let Some(dir) = current {
        if dir.join("pubspec.yaml").is_file() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    file.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// `.dart` files under `root`, without hidden directories, `build` or
/// subpackages (another `pubspec.yaml`), in order.
pub(crate) fn project_files(root: &Path) -> Vec<PathBuf> {
    let mut output = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<PathBuf> =
            entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if name.starts_with('.')
                    || name == "build"
                    || path.join("pubspec.yaml").is_file()
                {
                    continue;
                }
                stack.push(path);
            } else if name.ends_with(".dart") && output.len() < MAX_FILES {
                output.push(path);
            }
        }
    }
    output.sort();
    output
}

/// The text is a part (`part of`), which cannot come in as a library.
pub(crate) fn is_part(text: &str) -> bool {
    if !text.contains("part") {
        return false;
    }
    let mut names = dartforge_intern::Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    analyzed
        .unit
        .directives
        .iter()
        .any(|d| matches!(d.kind, ast::DirectiveKind::PartOf { .. }))
}

/// Canonical path (without `\\?\`) of the file of a `file:` URI.
pub(crate) fn uri_file(uri: &str) -> Option<PathBuf> {
    let file = Url::parse(uri).ok()?.to_file_path().ok()?;
    Some(dartforge_elements::config::without_verbatim(
        std::fs::canonicalize(&file).unwrap_or(file),
    ))
}

/// Loads the project that contains `uri` (all libraries under the root, as
/// entries of a single load), with the open documents in their current
/// texts, and infers the bodies of the project's libraries with local
/// registration.
pub(crate) fn load_project(
    sdk: &SdkLayout,
    documents: &DocumentStore,
    uri: &str,
) -> Option<Project> {
    load_project_with(sdk, documents, uri, None)
}

/// The synthetic file of the wide load: imports every SDK library and every
/// `lib/` file of the packages, so that they enter the program (the
/// analyzer's `knownFiles` after the first non-local search, §11.2).
const KNOWN: &str = ".dartforge_known.dart";

/// `(name, lib/ folder)` of the packages in `root`'s `package_config.json`.
pub(crate) fn packages_with_name(root: &Path) -> Vec<(String, PathBuf)> {
    let file = root.join(".dart_tool").join("package_config.json");
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    let Some(list) = json.get("packages").and_then(|v| v.as_array()) else { return Vec::new() };
    let Some(base) = file.parent() else { return Vec::new() };
    let resolve = |u: &str, relative_to: &Path| -> Option<PathBuf> {
        if let Ok(x) = Url::parse(u) {
            return x.to_file_path().ok();
        }
        Some(relative_to.join(u.trim_end_matches('/')))
    };
    let mut output = Vec::new();
    for p in list {
        let (Some(name), Some(root_uri)) = (p.get("name").and_then(|v| v.as_str()), p.get("rootUri").and_then(|v| v.as_str())) else { continue };
        let Some(package_root) = resolve(root_uri, base) else { continue };
        let package_uri = p.get("packageUri").and_then(|v| v.as_str()).unwrap_or("lib/");
        if let Some(lib) = resolve(package_uri, &package_root) {
            output.push((name.to_string(), lib));
        }
    }
    output
}

/// The wide load of the non-local reference search (docs/LSP-ESPECIFICACAO.md
/// §11.9 B): the project, plus every SDK library and every `lib/` file of
/// the packages, with the project's bodies inferred. Those of the outside
/// libraries that cite or declare a name come in through
/// [`Project::widen_to`].
pub(crate) fn load_wide_project(sdk: &SdkLayout, documents: &DocumentStore, uri: &str) -> Option<Project> {
    let file = uri_file(uri)?;
    let root = project_root(&file);
    let mut text = String::new();
    let mut sdk_libraries: Vec<&String> = sdk.libraries.keys().collect();
    sdk_libraries.sort();
    for b in sdk_libraries {
        text.push_str(&format!("import 'dart:{b}' as _sdk_{};\n", b.replace(|c: char| !c.is_ascii_alphanumeric(), "_")));
    }
    for (i, (package, lib)) in packages_with_name(&root).into_iter().enumerate() {
        let mut stack = vec![lib.clone()];
        let mut files = Vec::new();
        while let Some(d) = stack.pop() {
            let Ok(r) = std::fs::read_dir(&d) else { continue };
            for e in r.flatten() {
                let c = e.path();
                if c.is_dir() {
                    stack.push(c);
                } else if c.extension().is_some_and(|x| x == "dart") {
                    files.push(c);
                }
            }
        }
        files.sort();
        for (k, a) in files.into_iter().enumerate() {
            if std::fs::read_to_string(&a).is_ok_and(|t| is_part(&t)) {
                continue;
            }
            let Ok(rel) = a.strip_prefix(&lib) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/");
            text.push_str(&format!("import 'package:{package}/{rel}' as _pkg_{i}_{k};\n"));
        }
    }
    load_project_with(sdk, documents, uri, Some((root.join(KNOWN), text)))
}

impl Project {
    /// Infers the bodies of the libraries outside the inferred ones that have a
    /// file citing or declaring `name` (the candidates of `_addResults`), over
    /// the retained outline ([`Query::infer_libraries`]). The already inferred
    /// ones stay: in a retained wide load, each library is inferred once.
    pub(crate) fn widen_to(&mut self, name: &str) {
        let p = self.program();
        let mut extras: Vec<LibraryId> = Vec::new();
        for i in 0..p.units.len() {
            let u = UnitId(i as u32);
            let unit = p.unit(u);
            if self.libraries.contains(&unit.library) || extras.contains(&unit.library) || unit.role == dartforge_elements::model::UnitRole::Patch {
                continue;
            }
            if unit.path.as_deref().is_some_and(|c| c.ends_with(KNOWN)) {
                continue;
            }
            if !unit.source.contains(name) {
                continue;
            }
            if self.unit_referenced_names(u).contains(name) || declares(unit, &self.query.names, name) {
                extras.push(unit.library);
            }
        }
        if extras.is_empty() {
            return;
        }
        extras.sort();
        self.query.infer_libraries(&extras);
        self.libraries.extend(extras);
    }
}

/// The unit declares (at top level or as a member) something named `name`.
fn declares(unit: &dartforge_elements::model::Unit, names: &dartforge_intern::Interner, name: &str) -> bool {
    let a = &unit.ast;
    let n = |s: SymbolId| names.resolve(s) == name;
    a.decls.iter().any(|d| match &d.kind {
        DeclKind::Class(x) => n(x.name.sym),
        DeclKind::Mixin(x) => n(x.name.sym),
        DeclKind::Enum(x) => n(x.name.sym) || x.constants.iter().any(|k| n(k.name.sym)),
        DeclKind::Extension(x) => x.name.is_some_and(|k| n(k.sym)),
        DeclKind::ExtensionType(x) => n(x.name.sym),
        DeclKind::Typedef(x) => n(x.name.sym),
        DeclKind::Variables(l) => l.variables.iter().any(|v| n(v.name.sym)),
        _ => false,
    }) || a.functions.iter().any(|f| f.name.is_some_and(|k| n(k.sym)))
        || a.members.iter().any(|m| match &m.kind {
            MemberKind::Field(l) => l.variables.iter().any(|v| n(v.name.sym)),
            MemberKind::Constructor(k) => n(k.class_name.sym) || k.name.is_some_and(|x| n(x.sym)),
            MemberKind::Method(_) => false,
        })
}

fn load_project_with(
    sdk: &SdkLayout,
    documents: &DocumentStore,
    uri: &str,
    synthetic: Option<(PathBuf, String)>,
) -> Option<Project> {
    let file = uri_file(uri)?;
    let root = project_root(&file);
    let mut generator = SemanticAnalyzer::open(documents);
    let synthetic_path = synthetic.as_ref().map(|(c, _)| c.clone());
    if let Some((c, t)) = synthetic {
        generator.put(c, t, "lsp", vec![]);
    }
    let mut entries: Vec<PathBuf> = Vec::new();
    let text_of = |path: &Path| -> Option<String> {
        let open = Url::from_file_path(path)
            .ok()
            .and_then(|u| documents.get(u.as_str()).map(str::to_string));
        open.or_else(|| std::fs::read_to_string(path).ok())
    };
    let mut candidates = vec![file.clone()];
    candidates.extend(
        project_files(&root)
            .into_iter()
            .filter(|c| *c != file),
    );
    // Open documents under the root that do not exist on disk yet.
    for open in documents.uris() {
        if let Some(c) = uri_file(open)
            && c.starts_with(&root)
            && c.extension().is_some_and(|e| e == "dart")
            && !candidates.contains(&c)
        {
            candidates.push(c);
        }
    }
    for c in candidates {
        if text_of(&c).is_some_and(|t| !is_part(&t)) {
            entries.push(c);
        }
    }
    if let Some(c) = &synthetic_path {
        entries.push(c.clone());
    }
    if entries.is_empty() {
        // Only parts: the owning library is outside the root.
        return None;
    }
    let generation = generator.finish(1).ok()?;
    let mut names = dartforge_intern::Interner::new();
    let references: Vec<&Path> = entries.iter().map(PathBuf::as_path).collect();
    let (program, _) = dartforge_elements::load::load_lenient_entries(
        &references,
        sdk,
        None,
        &mut names,
        None,
        None,
        Some(generation),
    );
    let libraries: HashSet<LibraryId> = program
        .units
        .iter()
        .filter(|u| {
            !program.library(u.library).is_sdk
                && u.path.as_deref().is_some_and(|p| inside(p, &root) && Some(p) != synthetic_path.as_deref())
        })
        .map(|u| u.library)
        .collect();
    let mut list: Vec<LibraryId> = libraries.iter().copied().collect();
    list.sort();
    let query = Query::infer(program, names, &list, true, None);
    Some(Project {
        query,
        root,
        libraries,
        referenced_names: Default::default(),
        reverse_index: Default::default(),
        comments: Default::default(),
        occurrences_by_unit: Default::default(),
        written_subtypes: Default::default(),
    })
}

/// Loads only the library of `uri` (and what it imports), inferring its
/// bodies: enough for definition and hover, which look at one file.
pub(crate) fn load_library(
    sdk: &SdkLayout,
    documents: &DocumentStore,
    uri: &str,
) -> Option<Project> {
    let text = documents.get(uri)?;
    let file = uri_file(uri)?;
    let (program, names, unit) = crate::semantic::load(sdk, uri, text, Some(documents))?;
    let lib = program.unit(unit).library;
    let query = Query::infer(program, names, &[lib], true, None);
    Some(Project {
        query,
        root: project_root(&file),
        libraries: HashSet::from([lib]),
        referenced_names: Default::default(),
        reverse_index: Default::default(),
        comments: Default::default(),
        occurrences_by_unit: Default::default(),
        written_subtypes: Default::default(),
    })
}

/// `path` is under `root` (comparison by canonical keys).
pub(crate) fn inside(path: &Path, root: &Path) -> bool {
    dartforge_elements::generated::key(path).starts_with(dartforge_elements::generated::key(root))
}

/// Byte that can be part of a Dart identifier.
fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// The identifier that contains `offset` (or ends at it).
pub(crate) fn word(source: &str, offset: usize) -> Option<Span> {
    let b = source.as_bytes();
    let offset = offset.min(b.len());
    let mut start = offset;
    while start > 0 && is_ident(b[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < b.len() && is_ident(b[end]) {
        end += 1;
    }
    (start < end && !b[start].is_ascii_digit()).then_some(Span {
        start: start,
        end: end,
    })
}

/// Base name of a member: a setter is stored as `x=` (or `x_=`).
pub(crate) fn base_name(name: &str) -> &str {
    // Only a setter's `=` (`x=`): the `==` and `[]=` operators stay.
    name.strip_suffix("_=")
        .or_else(|| name.strip_suffix('=').filter(|r| !r.is_empty() && r.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')))
        .unwrap_or(name)
}

/// `offset` falls inside `s` (exclusive end).
fn contains(s: Span, offset: usize) -> bool {
    s.start <= offset && offset < s.end
}

impl Project {
    /// The loaded program.
    pub(crate) fn program(&self) -> &Program {
        &self.query.program
    }

    /// The text of a symbol.
    pub(crate) fn name(&self, s: SymbolId) -> &str {
        self.query.name(s)
    }

    /// Units of the inferred libraries, in order.
    pub(crate) fn units(&self) -> Vec<UnitId> {
        (0..self.program().units.len())
            .map(|i| UnitId(i as u32))
            .filter(|u| self.libraries.contains(&self.program().unit(*u).library))
            .collect()
    }

    /// [`Project::units`], only those of `filter` when there is one.
    pub(crate) fn units_in(&self, filter: Option<&HashSet<UnitId>>) -> Vec<UnitId> {
        let mut v = self.units();
        if let Some(f) = filter {
            v.retain(|u| f.contains(u));
        }
        v
    }

    /// The `file:` URI of a unit with a path.
    pub(crate) fn unit_uri(&self, u: UnitId) -> Option<String> {
        Url::from_file_path(self.program().unit(u).path.as_ref()?)
            .ok()
            .map(|u| u.to_string())
    }

    /// The unit loaded from the file of `uri` (canonical path).
    pub(crate) fn uri_unit(&self, uri: &str) -> Option<UnitId> {
        let path = uri_file(uri)?;
        let key = dartforge_elements::generated::key(&path);
        self.program()
            .units
            .iter()
            .position(|u| {
                u.path
                    .as_deref()
                    .map(dartforge_elements::generated::key)
                    .as_ref()
                    == Some(&key)
            })
            .map(|i| UnitId(i as u32))
    }

    /// The library belongs to the project (and not to the SDK or an external package).
    pub(crate) fn in_project(&self, lib: LibraryId) -> bool {
        let l = self.program().library(lib);
        !l.is_sdk
            && l.units.iter().all(|u| {
                self.program()
                    .unit(*u)
                    .path
                    .as_deref()
                    .is_some_and(|p| inside(p, &self.root))
            })
    }

    /// Library that declares a top-level element.
    pub(crate) fn element_library(&self, el: Element) -> LibraryId {
        let p = self.program();
        match el {
            Element::Class(c) => p.class(c).library,
            Element::Extension(x) => p.extension(x).library,
            Element::Typedef(t) => p.typedef(t).library,
            Element::Function(f) => p.function(f).library,
            Element::Variable(v) => p.variable(v).library,
            Element::Prefix(l, _) => l,
        }
    }

    // -----------------------------------------------------------------
    // What is under the cursor
    // -----------------------------------------------------------------

    /// What the position denotes. `Ok(None)`: there is no name there
    /// (whitespace, keyword, literal). `Err`: there is a name, but without
    /// resolution.
    pub(crate) fn identify(
        &self,
        unit: UnitId,
        offset: usize,
    ) -> Result<Option<Denoted>, String> {
        let u = self.program().unit(unit);
        let Some(name_) = word(&u.source, offset) else {
            return Ok(None);
        };
        let text = &u.source[name_.start..name_.end];
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let ast = &u.ast;
        let denoted = |target_: Target, expr: Option<ast::ExprId>, concrete: Option<Concrete>| {
            Ok(Some(Denoted {
                target: target_,
                name: name_,
                expr,
                concrete,
            }))
        };
        // References in expressions.
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let referred = match &e.kind {
                ExprKind::Identifier(n) => *n,
                ExprKind::Property { name, .. } => *name,
                ExprKind::InstanceCreation {
                    constructor: Some(n),
                    ..
                } => *n,
                _ => continue,
            };
            if referred.span != name_ {
                continue;
            }
            // A prefix that collides with a top-level declaration is a compile error
            // (`prefix_collides_with_top_level_member`): what `p` or `p.x` denotes is
            // not asserted.
            let prefixed_target = match &e.kind {
                ExprKind::Property { target, .. } => match &ast.expr(*target).kind {
                    ExprKind::Identifier(p) => Some(p.sym),
                    _ => None,
                },
                ExprKind::Identifier(n) => Some(n.sym),
                _ => None,
            };
            if let Some(p) = prefixed_target
                && self.program().library(u.library).prefixes.contains_key(&p)
                && self.program().library(u.library).declared.contains_key(&p)
            {
                return Err(format!(
                    "The prefix '{}' collides with a top-level declaration.",
                    self.name(p)
                ));
            }
            // `A.name(…)` and `A(…)` without `new`: the constructor resolution is on
            // the call, not on its target.
            let constructor = call_constructor(ast, bodies, id);
            if matches!(bodies.get_resolved(id), None | Some(Resolved::Dynamic))
                && constructor.is_none()
                && let Some(f) = self.object_member_on_dynamic(unit, id, Self::call_arguments(ast, id))
            {
                return denoted(self.function_member(f), Some(id), Some(self.function_concrete(f)));
            }
            let resolved = match (bodies.get_resolved(id), constructor) {
                (None, Some(f)) => Some(Resolved::Constructor(f)),
                (Some(Resolved::Element(Element::Class(c))), Some(f)) => {
                    return denoted(
                        Target::Top(Element::Class(*c)),
                        Some(id),
                        Some(Concrete::Function(f)),
                    );
                }
                (r, _) => r.cloned(),
            };
            return match resolved.as_ref() {
                Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {
                    match bodies.local_declaration(id) {
                        Some(d) => {
                            let (target_, concrete) = self.local_or_field(unit, d);
                            denoted(target_, Some(id), concrete)
                        }
                        None => Ok(None),
                    }
                }
                Some(Resolved::Element(Element::Prefix(_, p))) => denoted(
                    Target::Prefix {
                        library: u.library,
                        name: *p,
                    },
                    Some(id),
                    None,
                ),
                Some(Resolved::Prefix(_)) => {
                    let ExprKind::Identifier(n) = &e.kind else {
                        return Ok(None);
                    };
                    denoted(
                        Target::Prefix {
                            library: u.library,
                            name: n.sym,
                        },
                        Some(id),
                        None,
                    )
                }
                Some(Resolved::Element(el)) => {
                    let (target_, concrete) = self.top(*el);
                    denoted(target_, Some(id), concrete)
                }
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                })
                | Some(Resolved::ExtensionMember { member: f, .. }) => denoted(
                    self.function_member(*f),
                    Some(id),
                    Some(self.function_concrete(*f)),
                ),
                Some(Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                }) => denoted(
                    self.variable_member(*v),
                    Some(id),
                    Some(Concrete::Variable(*v)),
                ),
                Some(Resolved::Constructor(f)) => {
                    let fe = self.program().function(*f);
                    match fe.class {
                        Some(c) if self.name(self.program().class(c).name) == text => denoted(
                            Target::Top(Element::Class(c)),
                            Some(id),
                            Some(Concrete::Function(*f)),
                        ),
                        _ => denoted(Target::Constructor(*f), Some(id), Some(Concrete::Function(*f))),
                    }
                }
                Some(Resolved::TypeParameter(_)) => {
                    match type_parameter_declaration(ast, name_.start, referred.sym) {
                        Some(d) => denoted(
                            Target::TypeParameter {
                                unit,
                                declaration: d,
                            },
                            Some(id),
                            None,
                        ),
                        None => Ok(None),
                    }
                }
                _ => Err(format!("'{text}' is not resolved.")),
            };
        }
        // Declaration of a local or parameter.
        if bodies.local_type(name_.start).is_some() {
            let (target_, concrete) = self.local_or_field(unit, name_.start);
            return denoted(target_, None, concrete);
        }
        // Type annotations.
        for t in &ast.types {
            let ast::TypeKind::Named { name, .. } = &t.kind else {
                continue;
            };
            let Some(pos) = name.iter().position(|n| n.span == name_) else {
                continue;
            };
            let lib = u.library;
            // `A.name` as a type is the form of `new A.name()` and of `= A.name`: the
            // class and the named constructor, not a prefix and a type.
            if let [class, constructor] = &name[..]
                && !is_prefix(self.program(), lib, class.sym)
                && let Some(Element::Class(c)) = self
                    .program()
                    .lookup(lib, class.sym)
                    .and_then(|b| b.getter)
            {
                if pos == 0 {
                    let (target_, concrete) = self.top(Element::Class(c));
                    return denoted(target_, None, concrete);
                }
                return match self.program().class(c).constructors.get(&constructor.sym) {
                    Some(f) => denoted(Target::Constructor(*f), None, Some(Concrete::Function(*f))),
                    None => Err(format!("The constructor '{text}' is not resolved.")),
                };
            }
            if pos + 1 < name.len() {
                // `p` in `p.Name`.
                return denoted(
                    Target::Prefix {
                        library: lib,
                        name: name[pos].sym,
                    },
                    None,
                    None,
                );
            }
            if name.len() == 1
                && let Some(d) = type_parameter_declaration(ast, name_.start, name[0].sym)
            {
                return denoted(
                    Target::TypeParameter {
                        unit,
                        declaration: d,
                    },
                    None,
                    None,
                );
            }
            let binding = match &name[..] {
                [n] => self.program().lookup(lib, n.sym),
                [p, n] => self.program().lookup_prefixed(lib, p.sym, n.sym),
                _ => None,
            };
            return match binding.and_then(|b| b.getter) {
                Some(el) => {
                    let (target_, concrete) = self.top(el);
                    denoted(target_, None, concrete)
                }
                None => Err(format!("The type '{text}' is not resolved.")),
            };
        }
        // Prefix in the directive `import … as p`.
        for d in &u.unit.directives {
            if let ast::DirectiveKind::Import {
                prefix: Some(p), ..
            } = &d.kind
                && p.span == name_
            {
                return denoted(
                    Target::Prefix {
                        library: u.library,
                        name: p.sym,
                    },
                    None,
                    None,
                );
            }
        }
        // `show`/`hide` names of an import or export: the element the target
        // library exports with that name.
        {
            let p = self.program();
            let lib = p.library(u.library);
            let targets = lib
                .imports
                .iter()
                .filter(|i| i.unit == unit)
                .map(|i| (i.directive, i.library))
                .chain(lib.exports.iter().filter(|e| e.unit == unit).map(|e| (e.directive, e.library)));
            for (index, target_lib) in targets {
                let Some(d) = u.unit.directives.get(index) else { continue };
                let combinators_ = match &d.kind {
                    ast::DirectiveKind::Import { combinators, .. } | ast::DirectiveKind::Export { combinators, .. } => combinators,
                    _ => continue,
                };
                for c in combinators_ {
                    let (ast::Combinator::Show(names) | ast::Combinator::Hide(names)) = c;
                    if let Some(n) = names.iter().find(|n| n.span == name_)
                        && let Some(el) = p.library(target_lib).exported.get(&n.sym).and_then(|b| b.getter.or(b.setter))
                    {
                        let (target_, concrete) = self.top(el);
                        return denoted(target_, None, concrete);
                    }
                }
            }
        }
        // Metadata (`@name`, `@p.name`, `@Class.ctor(...)`).
        if let Some(d) = self.in_metadata(unit, name_) {
            return Ok(Some(d));
        }
        // `x = e` in a constructor initializer: the field.
        for (mi, m) in ast.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for start in k.initializers.iter() {
                if let ast::Initializer::Field { name, .. } = start
                    && name.span == name_
                    && let Some(c) = self.node_constructor(unit, ast::MemberId(mi as u32)).and_then(|f| self.program().function(f).class)
                    && let Some(v) = self.program().class(c).fields.iter().copied().find(|v| self.program().variable(*v).name == name.sym)
                {
                    return denoted(self.variable_member(v), None, Some(Concrete::Variable(v)));
                }
            }
        }
        // The `x:` label of a named argument: the declared parameter (a `this.x`
        // becomes the field), including of an SDK executable.
        if let Some(d) = self.in_argument_label(unit, name_) {
            return Ok(Some(d));
        }
        // References from documentation comments.
        if let Some(d) = self.in_dartdoc(unit, name_) {
            return Ok(Some(d));
        }
        // Declarations.
        if let Some((target_, concrete)) = self.declaration_in(unit, name_) {
            return denoted(target_, None, concrete);
        }
        Ok(None)
    }

    /// `Object`'s member that the name of the access `id` denotes when its
    /// target is `dynamic` (`MethodInvocationResolver._resolveReceiverDynamicBounded`
    /// and `TypePropertyResolver` on a dynamic receiver): an invoked method of
    /// `Object` whose positional arguments match its parameters
    /// (`_hasMatchingObjectMethod`: `toString()`, `noSuchMethod(i)`), or, without
    /// a call, a getter or a method tear-off of `Object`. `InvalidType` targets
    /// resolve to nothing.
    pub(crate) fn object_member_on_dynamic(&self, unit: UnitId, id: ast::ExprId, call: Option<&ast::Arguments>) -> Option<FunctionElementId> {
        let ast = &self.program().unit(unit).ast;
        let ExprKind::Property { target, name, .. } = &ast.expr(id).kind else { return None };
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let t = *bodies.static_types.get(target.0 as usize)?;
        if !matches!(self.query.table.get(t), dartforge_types::Type::Dynamic) || self.query.table.is_invalid(t) {
            return None;
        }
        let object = self.program().class(self.query.core.object_class?);
        let f = *object.instance_members.get(&name.sym)?;
        let kind = self.program().function(f).kind;
        match call {
            Some(arguments) => {
                let parameters = match self.name(name.sym) {
                    "toString" => 0,
                    "noSuchMethod" => 1,
                    _ => return None,
                };
                (kind == FunctionKind::Function && arguments.args.len() == parameters && arguments.args.iter().all(|a| a.name.is_none())).then_some(f)
            }
            None => matches!(kind, FunctionKind::Function | FunctionKind::Getter).then_some(f),
        }
    }

    /// The arguments of the call whose target is `id`, if any.
    fn call_arguments(ast: &Ast, id: ast::ExprId) -> Option<&ast::Arguments> {
        ast.exprs.iter().find_map(|e| match &e.kind {
            ExprKind::Call { target, arguments } if *target == id => Some(&**arguments),
            _ => None,
        })
    }

    /// Local at offset `declaration`; a `this.x` parameter becomes field `x`.
    fn local_or_field(&self, unit: UnitId, declaration: usize) -> (Target, Option<Concrete>) {
        let ast = &self.program().unit(unit).ast;
        if let Some((p, _)) = parameter_in(ast, declaration)
            && p.this_
            && let Some(r) = self.parameter_field(unit, declaration)
        {
            return r;
        }
        (
            Target::Local {
                unit,
                declaration,
            },
            None,
        )
    }

    /// The field initialized by the `this.x` parameter declared at `offset`.
    fn parameter_field(
        &self,
        unit: UnitId,
        offset: usize,
    ) -> Option<(Target, Option<Concrete>)> {
        let ast = &self.program().unit(unit).ast;
        let (p, _) = parameter_in(ast, offset)?;
        let symbol = p.name?.sym;
        let class = self.program().classes.iter().position(|c| {
            c.decl.is_some_and(|d| {
                d.unit == unit
                    && contains(self.program().unit(d.unit).ast.decl(d.decl).span, offset)
            })
        })?;
        let class = ClassId(class as u32);
        let field = self
            .program()
            .class(class)
            .fields
            .iter()
            .copied()
            .find(|v| self.program().variable(*v).name == symbol);
        Some((
            Target::Member {
                owner: Owner::Class(class),
                name: self.name(symbol).to_string(),
                is_static: false,
            },
            field.map(Concrete::Variable),
        ))
    }

    /// Target and concrete element of a top-level element (an implicit accessor
    /// becomes the variable).
    fn top(&self, el: Element) -> (Target, Option<Concrete>) {
        match el {
            Element::Function(f) => {
                let fe = self.program().function(f);
                if fe.kind == FunctionKind::ImplicitAccessor
                    && let Some(v) = fe.variable
                {
                    return (
                        Target::Top(Element::Variable(v)),
                        Some(Concrete::Variable(v)),
                    );
                }
                (Target::Top(el), Some(Concrete::Function(f)))
            }
            Element::Variable(v) => (Target::Top(el), Some(Concrete::Variable(v))),
            Element::Prefix(l, p) => (
                Target::Prefix {
                    library: l,
                    name: p,
                },
                None,
            ),
            _ => (Target::Top(el), None),
        }
    }

    /// The target of a function member (an implicit accessor becomes the variable).
    pub(crate) fn function_member(&self, f: FunctionElementId) -> Target {
        let fe = self.program().function(f);
        if let Some(v) = fe.variable
            && fe.kind == FunctionKind::ImplicitAccessor
        {
            return self.variable_member(v);
        }
        let name = base_name(self.name(fe.name)).to_string();
        let owner = match (fe.class, fe.extension) {
            (_, Some(x)) => Owner::Extension(x),
            (Some(c), _) => Owner::Class(c),
            (None, None) => return Target::Top(Element::Function(f)),
        };
        Target::Member {
            owner,
            name,
            is_static: fe.static_,
        }
    }

    /// The target of a field or enum constant (a top-level one becomes
    /// [`Target::Top`]).
    pub(crate) fn variable_member(&self, v: VariableId) -> Target {
        let ve = self.program().variable(v);
        let name = self.name(ve.name).to_string();
        let owner = match (ve.class, ve.extension) {
            (_, Some(x)) => Owner::Extension(x),
            (Some(c), _) => Owner::Class(c),
            (None, None) => return Target::Top(Element::Variable(v)),
        };
        Target::Member {
            owner,
            name,
            is_static: ve.static_,
        }
    }

    /// Declaration (top-level, member, constructor, enum constant, type
    /// parameter) whose name is `name`.
    fn declaration_in(&self, unit: UnitId, name: Span) -> Option<(Target, Option<Concrete>)> {
        let program = self.program();
        let u = program.unit(unit);
        let ast = &u.ast;
        for &d in &u.unit.declarations {
            let decl = ast.decl(d);
            let name_decl = match &decl.kind {
                DeclKind::Class(c) => Some(c.name),
                DeclKind::Mixin(m) => Some(m.name),
                DeclKind::Enum(e) => Some(e.name),
                DeclKind::ExtensionType(e) => Some(e.name),
                DeclKind::Typedef(t) => Some(t.name),
                DeclKind::Extension(x) => x.name,
                DeclKind::Function(f) => ast.function(*f).name,
                DeclKind::Variables(vl) => {
                    vl.variables.iter().map(|v| v.name).find(|n| n.span == name)
                }
            };
            if let Some(n) = name_decl.filter(|n| n.span == name) {
                let el = match &decl.kind {
                    DeclKind::Function(fid) => {
                        self.node_function(unit, *fid).map(Element::Function)
                    }
                    DeclKind::Variables(vl) => {
                        let i = vl.variables.iter().position(|v| v.name.span == name)?;
                        self.node_variable(VariableRef::TopLevel {
                            unit: unit,
                            decl: d,
                            index: i,
                        })
                        .map(Element::Variable)
                    }
                    _ => program
                        .library(u.library)
                        .declared
                        .get(&n.sym)
                        .and_then(|b| b.getter.or(b.setter)),
                };
                return el.map(|el| self.top(el));
            }
            if let DeclKind::Enum(e) = &decl.kind {
                for (i, c) in e.constants.iter().enumerate() {
                    if c.name.span == name
                        && let Some(v) = self.node_variable(VariableRef::EnumConstant {
                            unit: unit,
                            decl: d,
                            index: i,
                        })
                    {
                        return Some((self.variable_member(v), Some(Concrete::Variable(v))));
                    }
                }
            }
        }
        for (mi, m) in ast.members.iter().enumerate() {
            let mid = ast::MemberId(mi as u32);
            match &m.kind {
                MemberKind::Method(fid) => {
                    if ast.function(*fid).name.is_some_and(|n| n.span == name)
                        && let Some(f) = self.node_function(unit, *fid)
                    {
                        return Some((self.function_member(f), Some(Concrete::Function(f))));
                    }
                }
                MemberKind::Field(vl) => {
                    for (i, v) in vl.variables.iter().enumerate() {
                        if v.name.span == name
                            && let Some(vid) = self.node_variable(VariableRef::Field {
                                unit: unit,
                                member: mid,
                                index: i,
                            })
                        {
                            return Some((
                                self.variable_member(vid),
                                Some(Concrete::Variable(vid)),
                            ));
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    let f = self.node_constructor(unit, mid);
                    if k.class_name.span == name {
                        // `ElementLocator`: the `returnType` of a constructor declaration denotes
                        // the constructor (the unnamed one too).
                        return f.map(|f| (Target::Constructor(f), Some(Concrete::Function(f))));
                    }
                    if k.name.is_some_and(|n| n.span == name) {
                        let f = f?;
                        return Some((Target::Constructor(f), Some(Concrete::Function(f))));
                    }
                }
            }
        }
        // Declared type parameters.
        let declares = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.span == name);
        let in_type_parameters = ast.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(c) => declares(&c.type_params),
            DeclKind::Mixin(m) => declares(&m.type_params),
            DeclKind::Enum(e) => declares(&e.type_params),
            DeclKind::Extension(x) => declares(&x.type_params),
            DeclKind::ExtensionType(x) => declares(&x.type_params),
            DeclKind::Typedef(t) => declares(&t.type_params),
            _ => false,
        }) || ast.functions.iter().any(|f| declares(&f.type_params))
            || ast.types.iter().any(|t| {
                matches!(&t.kind, ast::TypeKind::Function { type_params, .. } if declares(type_params))
            });
        if in_type_parameters {
            return Some((
                Target::TypeParameter {
                    unit,
                    declaration: name.start,
                },
                None,
            ));
        }
        None
    }

    /// The named argument label at `name`: the parameter of the called executable.
    fn in_argument_label(&self, unit_: UnitId, name: Span) -> Option<Denoted> {
        let p = self.program();
        let u = p.unit(unit_);
        let ast = &u.ast;
        let bodies = &self.query.bodies.units[unit_.0 as usize];
        for (i, e) in ast.exprs.iter().enumerate() {
            let (arguments_, target_) = match &e.kind {
                ExprKind::Call { target, arguments } => (&**arguments, Some(*target)),
                ExprKind::InstanceCreation { arguments, .. } => (&**arguments, None),
                _ => continue,
            };
            let Some(label) = arguments_.args.iter().filter_map(|a| a.name).find(|n| n.span == name) else { continue };
            let call = [Some(ast::ExprId(i as u32)), target_].into_iter().flatten().find_map(|x| match bodies.get_resolved(x) {
                Some(Resolved::Constructor(f))
                | Some(Resolved::Element(Element::Function(f)))
                | Some(Resolved::Member { member: MemberRef::Function(f), .. })
                | Some(Resolved::ExtensionMember { member: f, .. }) => Some(*f),
                _ => None,
            })?;
            let fe = p.function(call);
            let parameters: (UnitId, &[ast::Parameter]) = match fe.node {
                FunctionRef::Function { unit, function } => (unit, p.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[])),
                FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => (unit, &k.parameters[..]),
                    _ => return None,
                },
                FunctionRef::None => return None,
            };
            let (pu, list) = parameters;
            let q = list.iter().find(|q| q.kind == ast::ParameterKind::Named && q.name.is_some_and(|n| n.sym == label.sym))?;
            let decl = q.name?.span.start;
            let (target_, concrete) = self.local_or_field(pu, decl);
            return Some(Denoted { target: target_, name, expr: None, concrete });
        }
        None
    }

    /// Name under the cursor in a metadata annotation.
    fn in_metadata(&self, unit: UnitId, name: Span) -> Option<Denoted> {
        let u = self.program().unit(unit);
        for a in metadata(&u.ast, &u.unit) {
            let Some(pos) = a.name.iter().position(|n| n.span == name) else {
                continue;
            };
            let parts: Vec<SymbolId> = a.name.iter().map(|n| n.sym).collect();
            let (mut target, mut concrete) = self.resolve_path(u.library, None, &parts[..=pos])?;
            // `@A(...)`: the annotation's name, without a constructor name, denotes the
            // `annotation.element`, the unnamed constructor.
            if a.name.len() == 1
                && a.arguments.is_some()
                && let Target::Top(Element::Class(c)) = target
                && let Some(f) = self
                    .program()
                    .class(c)
                    .constructors
                    .iter()
                    .find(|(s, _)| self.name(**s).is_empty())
                    .map(|(_, f)| *f)
            {
                target = Target::Constructor(f);
                concrete = Some(Concrete::Function(f));
            }
            return Some(Denoted {
                target,
                name,
                expr: None,
                concrete,
            });
        }
        None
    }

    /// Name under the cursor in a documentation `[…]` reference.
    fn in_dartdoc(&self, unit: UnitId, name: Span) -> Option<Denoted> {
        let u = self.program().unit(unit);
        for c in dartdoc::comments(&u.source) {
            if !contains(c.span, name.start) {
                continue;
            }
            for r in &c.references {
                let Some(pos) = r.iter().position(|(s, _)| *s == name) else {
                    continue;
                };
                let (target, concrete) = self.resolve_doc_reference(unit, c.span, &r[..=pos])?;
                return Some(Denoted {
                    target,
                    name,
                    expr: None,
                    concrete,
                });
            }
        }
        None
    }

    /// Resolves `a`, `a.b`, `a.b.c` from the scope of library `lib` (metadata
    /// and documentation): prefix, top-level element, static or instance member
    /// and named constructor. With `class`, a simple name is looked up first
    /// among its members (member documentation).
    fn resolve_path(
        &self,
        lib: LibraryId,
        class: Option<ClassId>,
        parts: &[SymbolId],
    ) -> Option<(Target, Option<Concrete>)> {
        let p = self.program();
        let (first, rest) = parts.split_first()?;
        if rest.is_empty()
            && let Some(c) = class
            && let Some(r) = self.member_by_name(c, *first)
        {
            return Some(r);
        }
        let mut current: Option<Element> = None;
        let mut rest = rest;
        if is_prefix(p, lib, *first) {
            if rest.is_empty() {
                return Some((
                    Target::Prefix {
                        library: lib,
                        name: *first,
                    },
                    None,
                ));
            }
            current = p.lookup_prefixed(lib, *first, rest[0])?.getter;
            rest = &rest[1..];
        } else {
            current = current.or(p.lookup(lib, *first).and_then(|b| b.getter.or(b.setter)));
        }
        let el = current?;
        match rest {
            [] => Some(self.top(el)),
            [member] => match el {
                Element::Class(c) => {
                    if let Some(f) = p.class(c).constructors.get(member).copied() {
                        return Some((Target::Constructor(f), Some(Concrete::Function(f))));
                    }
                    self.member_by_name(c, *member)
                }
                Element::Extension(x) => {
                    let ext = p.extension(x);
                    let f = ext
                        .static_members
                        .get(member)
                        .or_else(|| ext.instance_members.get(member))
                        .copied()?;
                    Some((self.function_member(f), Some(Concrete::Function(f))))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Member `name` of `c` (static, instance or inherited), with the concrete one.
    fn member_by_name(&self, c: ClassId, name: SymbolId) -> Option<(Target, Option<Concrete>)> {
        let p = self.program();
        let mut classes = vec![c];
        let mut seen = HashSet::new();
        while let Some(x) = classes.pop() {
            if !seen.insert(x) {
                continue;
            }
            let cl = p.class(x);
            if let Some(f) = cl
                .static_members
                .get(&name)
                .or_else(|| cl.instance_members.get(&name))
                .copied()
            {
                return Some((self.function_member(f), Some(self.function_concrete(f))));
            }
            if x == c
                && let Some(v) = cl
                    .fields
                    .iter()
                    .chain(&cl.enum_constants)
                    .copied()
                    .find(|v| p.variable(*v).name == name)
            {
                return Some((self.variable_member(v), Some(Concrete::Variable(v))));
            }
            classes.extend(cl.supertype_class);
            classes.extend(cl.mixin_classes.iter().copied());
            classes.extend(cl.interface_classes.iter().copied());
        }
        None
    }

    /// An implicit accessor becomes the variable; the rest stays the function.
    pub(crate) fn function_concrete(&self, f: FunctionElementId) -> Concrete {
        let fe = self.program().function(f);
        match fe.variable {
            Some(v) if fe.kind == FunctionKind::ImplicitAccessor => Concrete::Variable(v),
            _ => Concrete::Function(f),
        }
    }

    /// The declaration documented by comment `comment`: the first declaration,
    /// member or enum constant that starts after it.
    fn documented(&self, unit: UnitId, comment: Span) -> Option<Documented> {
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let mut best: Option<(usize, Documented)> = None;
        let mut consider = |start: usize, d: Documented| {
            if start >= comment.end && best.as_ref().is_none_or(|(i, _)| start < *i) {
                best = Some((start, d));
            }
        };
        for (i, d) in ast.decls.iter().enumerate() {
            consider(d.span.start, Documented::Decl(ast::DeclId(i as u32)));
        }
        for (i, m) in ast.members.iter().enumerate() {
            consider(m.span.start, Documented::Member(ast::MemberId(i as u32)));
        }
        best.map(|(_, d)| d)
    }

    /// Resolves a documentation reference (`[a]`, `[A.b]`, `[p.A.b]`) in the
    /// scope of the documented declaration: its parameters and type parameters,
    /// the members of the enclosing class and the library scope.
    pub(crate) fn resolve_doc_reference(
        &self,
        unit: UnitId,
        comment: Span,
        parts: &[(Span, String)],
    ) -> Option<(Target, Option<Concrete>)> {
        // `[A.new]`: the `new` after the dot becomes an identifier, the unnamed
        // constructor (`_parseOneCommentReference`).
        if let [first, second] = parts
            && second.1 == "new"
        {
            let (target, _) = self.resolve_doc_reference(unit, comment, std::slice::from_ref(first))?;
            let Target::Top(Element::Class(c)) = target else { return None };
            let empty = self.query.names.lookup("")?;
            let f = *self.program().class(c).constructors.get(&empty)?;
            return Some((Target::Constructor(f), Some(Concrete::Function(f))));
        }
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let symbols: Vec<SymbolId> = parts
            .iter()
            .map(|(s, _)| {
                // The name was already interned if it appears in the program; otherwise it
                // does not resolve.
                self.query.names.lookup(&u.source[s.start..s.end])
            })
            .collect::<Option<Vec<_>>>()?;
        let documented = self.documented(unit, comment);
        // A simple name: the declaration's parameters and type parameters.
        if let [symbol] = symbols[..] {
            let (function, types): (Option<&[Parameter]>, Vec<&ast::TypeParameter>) =
                match documented {
                    Some(Documented::Decl(d)) => match &ast.decl(d).kind {
                        DeclKind::Function(f) => (
                            ast.function(*f).parameters.as_deref(),
                            ast.function(*f).type_params.iter().collect(),
                        ),
                        DeclKind::Class(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Mixin(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Enum(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Extension(c) => (None, c.type_params.iter().collect()),
                        DeclKind::ExtensionType(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Typedef(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Variables(_) => (None, Vec::new()),
                    },
                    Some(Documented::Member(m)) => match &ast.member(m).kind {
                        MemberKind::Method(f) => (
                            ast.function(*f).parameters.as_deref(),
                            ast.function(*f).type_params.iter().collect(),
                        ),
                        MemberKind::Constructor(k) => (Some(&k.parameters[..]), Vec::new()),
                        MemberKind::Field(_) => (None, Vec::new()),
                    },
                    None => (None, Vec::new()),
                };
            if let Some(p) = function
                .into_iter()
                .flatten()
                .find(|p| p.name.is_some_and(|n| n.sym == symbol))
                && let Some(n) = p.name
            {
                return Some(self.local_or_field(unit, n.span.start));
            }
            if let Some(t) = types.iter().find(|t| t.name.sym == symbol) {
                return Some((
                    Target::TypeParameter {
                        unit,
                        declaration: t.name.span.start,
                    },
                    None,
                ));
            }
        }
        // Enclosing class (or the documented class itself).
        let offset = match documented {
            Some(Documented::Decl(d)) => ast.decl(d).span.start,
            Some(Documented::Member(m)) => ast.member(m).span.start,
            None => comment.end,
        };
        let class = self
            .program()
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.decl.is_some_and(|d| {
                    d.unit == unit && {
                        let s = ast.decl(d.decl).span;
                        s.start <= offset && offset < s.end
                    }
                })
            })
            .map(|(i, _)| ClassId(i as u32))
            .next();
        self.resolve_path(u.library, class, &symbols)
    }

    /// Function element declared by node `fid` of the unit.
    pub(crate) fn node_function(
        &self,
        unit: UnitId,
        fid: ast::FunctionId,
    ) -> Option<FunctionElementId> {
        self.program()
            .functions
            .iter()
            .position(|f| {
                f.node
                    == FunctionRef::Function {
                        unit: unit,
                        function: fid,
                    }
            })
            .map(|i| FunctionElementId(i as u32))
    }

    /// Element of the constructor declared by member `mid`.
    pub(crate) fn node_constructor(
        &self,
        unit: UnitId,
        mid: ast::MemberId,
    ) -> Option<FunctionElementId> {
        self.program()
            .functions
            .iter()
            .position(|f| {
                f.node
                    == FunctionRef::Constructor {
                        unit: unit,
                        member: mid,
                    }
            })
            .map(|i| FunctionElementId(i as u32))
    }

    /// The element of the variable declared at node `node`.
    fn node_variable(&self, node: VariableRef) -> Option<VariableId> {
        self.program()
            .variables
            .iter()
            .position(|v| v.node == node)
            .map(|i| VariableId(i as u32))
    }

    // -----------------------------------------------------------------
    // Declarations
    // -----------------------------------------------------------------

    /// Unit and name span of a top-level variable, field or constant.
    pub(crate) fn variable_name(&self, v: VariableId) -> Option<(UnitId, Span)> {
        let p = self.program();
        match p.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => {
                match &p.unit(unit).ast.decl(decl).kind {
                    DeclKind::Variables(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &p.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => {
                match &p.unit(unit).ast.decl(decl).kind {
                    DeclKind::Enum(e) => Some((unit, e.constants.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Representation { unit, decl } => match &p.unit(unit).ast.decl(decl).kind {
                DeclKind::ExtensionType(x) => Some((unit, x.representation_name.span)),
                _ => None,
            },
            VariableRef::None => None,
        }
    }

    /// Unit and name span of a function (top-level, method, constructor).
    pub(crate) fn function_name(&self, f: FunctionElementId) -> Option<(UnitId, Span)> {
        let p = self.program();
        let fe = p.function(f);
        match fe.node {
            FunctionRef::Function { unit, function } => {
                Some((unit, p.unit(unit).ast.function(function).name?.span))
            }
            FunctionRef::Constructor { unit, member } => {
                match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => Some((unit, k.name.unwrap_or(k.class_name).span)),
                    _ => None,
                }
            }
            FunctionRef::None => match (fe.variable, fe.class) {
                (Some(v), _) => self.variable_name(v),
                // Synthetic constructor: the class.
                (None, Some(c)) => self.top_element_name(Element::Class(c)),
                _ => None,
            },
        }
    }

    /// Unit and name span of a top-level element.
    pub(crate) fn top_element_name(&self, el: Element) -> Option<(UnitId, Span)> {
        let p = self.program();
        match el {
            Element::Class(c) => {
                let d = p.class(c).decl?;
                let n = match &p.unit(d.unit).ast.decl(d.decl).kind {
                    DeclKind::Class(x) => x.name,
                    DeclKind::Mixin(x) => x.name,
                    DeclKind::Enum(x) => x.name,
                    DeclKind::ExtensionType(x) => x.name,
                    _ => return None,
                };
                Some((d.unit, n.span))
            }
            Element::Typedef(t) => {
                let d = p.typedef(t).decl;
                match &p.unit(d.unit).ast.decl(d.decl).kind {
                    DeclKind::Typedef(x) => Some((d.unit, x.name.span)),
                    _ => None,
                }
            }
            Element::Extension(x) => {
                let d = p.extension(x).decl;
                match &p.unit(d.unit).ast.decl(d.decl).kind {
                    DeclKind::Extension(xd) => Some((d.unit, xd.name?.span)),
                    _ => None,
                }
            }
            Element::Function(f) => self.function_name(f),
            Element::Variable(v) => self.variable_name(v),
            Element::Prefix(l, s) => self.prefix_declaration(l, s),
        }
    }

    /// The `as p` of the library's first import with that prefix.
    pub(crate) fn prefix_declaration(
        &self,
        lib: LibraryId,
        name: SymbolId,
    ) -> Option<(UnitId, Span)> {
        let p = self.program();
        p.library(lib).imports.iter().find_map(|i| {
            if i.prefix != Some(name) {
                return None;
            }
            match &p.unit(i.unit).unit.directives.get(i.directive)?.kind {
                ast::DirectiveKind::Import {
                    prefix: Some(n), ..
                } => Some((i.unit, n.span)),
                _ => None,
            }
        })
    }

    /// Where the denoted element is declared: the concrete element when there is
    /// one (the resolved member, not the family), otherwise the target.
    pub(crate) fn declaration(&self, d: &Denoted) -> Option<(UnitId, Span)> {
        match (&d.target, d.concrete) {
            (Target::Top(Element::Class(c)), Some(Concrete::Function(f)))
                if self.program().function(f).kind == FunctionKind::Constructor =>
            {
                // `A()` with a written constructor goes to it.
                self.function_name(f)
                    .or_else(|| self.top_element_name(Element::Class(*c)))
            }
            (_, Some(Concrete::Function(f))) => self.function_name(f),
            (_, Some(Concrete::Variable(v))) => self.variable_name(v),
            (target, None) => self.target_declaration(target),
        }
    }

    /// `textDocument/definition`: the `DartNavigationComputer` visits only the
    /// node at the offset (`_getNavigationTargetNode`: the outermost node that
    /// starts there), so a few names navigate differently from what they
    /// denote: the name of a `for (… x in …)` variable has no region; the
    /// class name of a constructor declaration is the class (the unnamed
    /// constructor that starts at its name visits the whole declaration and
    /// keeps the constructor); an annotation's name is the class (the
    /// identifier's element, not `annotation.element`); and a `super.x`
    /// parameter is the parameter of the super constructor.
    pub(crate) fn definition(&self, unit: UnitId, offset: usize) -> Option<(UnitId, Span)> {
        let d = self.identify(unit, offset).ok()??;
        let p = self.program();
        let u = p.unit(unit);
        let ast = &u.ast;
        if for_in_variable(ast, d.name) {
            return None;
        }
        for (mi, m) in ast.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            if k.class_name.span != d.name {
                continue;
            }
            let starts_at_name = m.span.start == k.class_name.span.start && m.metadata.is_empty() && !doc_comment_before(&u.source, m.span.start);
            if k.name.is_none() && starts_at_name {
                break;
            }
            let class = self.node_constructor(unit, ast::MemberId(mi as u32)).and_then(|f| p.function(f).class)?;
            return self.top_element_name(Element::Class(class));
        }
        for a in metadata(ast, &u.unit) {
            if a.name.first().is_some_and(|n| n.span == d.name)
                && let Some(Element::Class(c)) = p.lookup(u.library, a.name[0].sym).and_then(|b| b.getter)
            {
                return self.top_element_name(Element::Class(c));
            }
        }
        if let Target::Local { declaration, .. } = d.target
            && declaration == d.name.start
            && let Some(target) = self.super_parameter_target(unit, declaration)
        {
            return Some(target);
        }
        self.declaration(&d)
    }

    /// The parameter of the super constructor that the `super.x` parameter
    /// declared at `declaration` forwards to (`superConstructorParameter`):
    /// the named one with the same name, or the positional one at the same
    /// position among the positional `super.` parameters.
    fn super_parameter_target(&self, unit: UnitId, declaration: usize) -> Option<(UnitId, Span)> {
        use dartforge_elements::model::FunctionRef;
        let p = self.program();
        let ast = &p.unit(unit).ast;
        let (parameter, owner) = parameter_in(ast, declaration)?;
        if !parameter.super_ {
            return None;
        }
        let OwnerParameter::Constructor(m) = owner else { return None };
        let MemberKind::Constructor(k) = &ast.member(m).kind else { return None };
        let sub = self.node_constructor(unit, m)?;
        let sup = self.super_constructor(sub)?;
        let (su, parameters): (UnitId, &[Parameter]) = match p.function(sup).node {
            FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(x) => (unit, &x.parameters[..]),
                _ => return None,
            },
            _ => return None,
        };
        let name = parameter.name?;
        let target = if parameter.kind == ast::ParameterKind::Named {
            parameters.iter().find(|q| q.kind == ast::ParameterKind::Named && q.name.is_some_and(|n| self.name(n.sym) == self.name(name.sym)))?
        } else {
            let position = k.parameters.iter().take_while(|q| q.name.is_none_or(|n| n.span.start != declaration)).filter(|q| q.super_ && q.kind != ast::ParameterKind::Named).count();
            parameters.iter().filter(|q| q.kind != ast::ParameterKind::Named).nth(position)?
        };
        Some((su, target.name?.span))
    }

    /// The declaration that `references` adds (`_getDeclarations`, §11.5): that
    /// of the element of step 6, through `nonSynthetic`. The `A` of `A()` is the
    /// class; a declared setter becomes the homonymous getter (a synthetic field
    /// gives `getter ?? setter`); an implicit accessor becomes the variable.
    pub(crate) fn declaration_to_references(&self, d: &Denoted) -> Option<(UnitId, Span)> {
        let p = self.program();
        if let Target::Top(Element::Class(c)) = &d.target {
            return self.top_element_name(Element::Class(*c));
        }
        if let Some(Concrete::Function(f)) = d.concrete {
            let fe = p.function(f);
            match fe.kind {
                FunctionKind::ImplicitAccessor => {
                    if let Some(v) = fe.variable {
                        return self.variable_name(v);
                    }
                }
                FunctionKind::Setter => {
                    let base = base_name(self.name(fe.name)).to_string();
                    let getter = match (fe.class, fe.extension) {
                        (Some(c), _) => p.class(c).instance_members.values().chain(p.class(c).static_members.values()).copied().find(|&g| {
                            p.function(g).kind == FunctionKind::Getter && self.name(p.function(g).name) == base
                        }),
                        (_, Some(x)) => p.extension(x).instance_members.values().chain(p.extension(x).static_members.values()).copied().find(|&g| {
                            p.function(g).kind == FunctionKind::Getter && self.name(p.function(g).name) == base
                        }),
                        _ => match self.query.names.lookup(&base).and_then(|s| p.lookup(fe.library, s)).and_then(|b| b.getter) {
                            Some(Element::Function(g)) if p.function(g).kind == FunctionKind::Getter => Some(g),
                            _ => None,
                        },
                    };
                    if let Some(g) = getter {
                        return self.function_name(g);
                    }
                }
                _ => {}
            }
        }
        self.declaration(d)
    }

    /// Where the target is declared (the first declaration, in the family).
    pub(crate) fn target_declaration(&self, target: &Target) -> Option<(UnitId, Span)> {
        match target {
            Target::Local {
                unit,
                declaration,
            }
            | Target::TypeParameter {
                unit,
                declaration,
            } => {
                let source = &self.program().unit(*unit).source;
                Some((*unit, word(source, *declaration)?))
            }
            Target::Top(el) => self.top_element_name(*el),
            Target::Constructor(f) => self.function_name(*f),
            Target::Prefix { library, name } => self.prefix_declaration(*library, *name),
            Target::Member {
                owner,
                name,
                is_static,
            } => {
                let p = self.program();
                let (functions, variables): (Vec<FunctionElementId>, Vec<VariableId>) = match owner {
                    Owner::Class(c) => {
                        let cl = p.class(*c);
                        (
                            self.declared(*c, name, *is_static),
                            cl.fields
                                .iter()
                                .chain(&cl.enum_constants)
                                .copied()
                                .filter(|v| self.name(p.variable(*v).name) == name)
                                .collect(),
                        )
                    }
                    Owner::Extension(x) => {
                        let ext = p.extension(*x);
                        let map = if *is_static {
                            &ext.static_members
                        } else {
                            &ext.instance_members
                        };
                        let mut f: Vec<FunctionElementId> = map
                            .iter()
                            .filter(|(s, _)| base_name(self.name(**s)) == name)
                            .map(|(_, f)| *f)
                            .collect();
                        f.sort();
                        (f, Vec::new())
                    }
                };
                variables
                    .into_iter()
                    .find_map(|v| self.variable_name(v))
                    .or_else(|| functions.into_iter().find_map(|f| self.function_name(f)))
            }
        }
    }

    /// All the names that declare the target (the whole family for a member;
    /// homonymous getter and setter; the `as p` of each import of a prefix).
    pub(crate) fn declarations(&self, target: &Target) -> BTreeSet<(UnitId, usize, usize)> {
        let mut output = BTreeSet::new();
        let mut by = |u: UnitId, s: Span| {
            output.insert((u, s.start, s.end));
        };
        match target {
            Target::Member {
                owner,
                name,
                is_static,
            } => {
                if let Ok(family) = self.family(*owner, name, *is_static, false) {
                    for f in &family.functions {
                        if let Some((u, s)) = self.function_name(*f) {
                            by(u, s);
                        }
                    }
                    for v in &family.variables {
                        if let Some((u, s)) = self.variable_name(*v) {
                            by(u, s);
                        }
                    }
                }
            }
            Target::Top(el) => {
                for e in self.top_elements(*el) {
                    if let Some((u, s)) = self.top_element_name(e) {
                        by(u, s);
                    }
                }
            }
            Target::Prefix { library, name } => {
                let p = self.program();
                for i in &p.library(*library).imports {
                    if i.prefix == Some(*name)
                        && let Some(d) = p.unit(i.unit).unit.directives.get(i.directive)
                        && let ast::DirectiveKind::Import {
                            prefix: Some(n), ..
                        } = &d.kind
                    {
                        by(i.unit, n.span);
                    }
                }
            }
            _ => {
                if let Some((u, s)) = self.target_declaration(target) {
                    by(u, s);
                }
            }
        }
        output
    }

    // -----------------------------------------------------------------
    // Occurrences
    // -----------------------------------------------------------------

    /// All the occurrences of the target in the inferred libraries
    /// (declarations included), as `(unit, start, end)`, sorted and without
    /// repetition.
    ///
    /// `refuse_external`: a member whose family includes a declaration outside
    /// the project is an error (rename); references accept it.
    ///
    /// # Errors
    ///
    /// With `refuse_external`, the message explaining why the member's family
    /// cannot change.
    pub(crate) fn occurrences(
        &self,
        target: &Target,
        refuse_external: bool,
    ) -> Result<BTreeSet<(UnitId, usize, usize)>, String> {
        self.occurrences_in(target, refuse_external, None)
    }

    /// [`Project::occurrences`] only in the units of `filter` (when there is
    /// one): the scans of expressions, types and documentation of members,
    /// top-level elements and constructors only look at them. The reference
    /// search passes the candidate files of `_addResults`, which is where it
    /// accepts uses (docs/LSP-ESPECIFICACAO.md §11.2): its result is the same,
    /// without walking the other files.
    pub(crate) fn occurrences_in(
        &self,
        target: &Target,
        refuse_external: bool,
        filter: Option<&HashSet<UnitId>>,
    ) -> Result<BTreeSet<(UnitId, usize, usize)>, String> {
        self.occurrences_with(target, refuse_external, filter, false)
    }

    /// [`Project::occurrences`] as the member rename sees them
    /// (`RenameClassMemberRefactoringImpl.fillChange`): the references of the
    /// `this.x` parameters of every renamed field, not only of the pivot's.
    pub(crate) fn rename_occurrences(&self, target: &Target) -> Result<BTreeSet<(UnitId, usize, usize)>, String> {
        self.occurrences_with(target, false, None, true)
    }

    fn occurrences_with(
        &self,
        target: &Target,
        refuse_external: bool,
        filter: Option<&HashSet<UnitId>>,
        family_parameters: bool,
    ) -> Result<BTreeSet<(UnitId, usize, usize)>, String> {
        let mut output = BTreeSet::new();
        let mut by = |u: UnitId, s: Span| {
            output.insert((u, s.start, s.end));
        };
        match target {
            Target::Local {
                unit,
                declaration,
            } => {
                for (u, s) in self.local_occurrences(*unit, *declaration, refuse_external)? {
                    by(u, s);
                }
            }
            Target::Top(el) => self.top_occurrences(*el, filter, &mut by),
            Target::Member {
                owner,
                name,
                is_static,
            } => self.member_occurrences(*owner, name, *is_static, refuse_external, family_parameters, filter, &mut by)?,
            Target::Constructor(f) => self.constructor_occurrences(*f, filter, &mut by),
            Target::Prefix { library, name } => {
                self.prefix_occurrences(*library, *name, &mut by)
            }
            Target::TypeParameter {
                unit,
                declaration,
            } => self.type_parameter_occurrences(*unit, *declaration, &mut by),
        }
        // Documentation references: what the reference resolves to must be one
        // of the elements searched (`_getRefElements`), so a member of a class the
        // index does not see as a subtype (`with MapMixin`) stays out even when
        // its own family reaches the target.
        let family = match target {
            Target::Member { owner, name, is_static } => self.family(*owner, name, *is_static, false).ok(),
            _ => None,
        };
        let matches_ = |a: &Target, concrete: Option<Concrete>| match (&family, concrete) {
            (Some(f), Some(Concrete::Function(x))) => f.functions.contains(&x),
            (Some(f), Some(Concrete::Variable(x))) => f.variables.contains(&x),
            _ => self.same_target(a, target),
        };
        for (u, s) in self.documentation_references(target, filter, matches_, true) {
            by(u, s);
        }
        Ok(output)
    }

    /// Two targets denote the same thing (a member, by family).
    fn same_target(&self, a: &Target, b: &Target) -> bool {
        match (a, b) {
            (
                Target::Member {
                    owner: d1,
                    name: n1,
                    is_static: e1,
                },
                Target::Member {
                    owner: d2,
                    name: n2,
                    is_static: e2,
                },
            ) => {
                if n1 != n2 || e1 != e2 {
                    return false;
                }
                if d1 == d2 {
                    return true;
                }
                match (
                    self.family(*d1, n1, *e1, false),
                    self.family(*d2, n2, *e2, false),
                ) {
                    (Ok(f1), Ok(f2)) => {
                        !f1.functions.is_disjoint(&f2.functions)
                            || !f1.variables.is_disjoint(&f2.variables)
                    }
                    _ => false,
                }
            }
            (Target::Top(x), Target::Top(y)) => x == y || self.top_elements(*x).contains(y),
            _ => a == b,
        }
    }

    /// Local, parameter or local function: the declaration, the expressions that
    /// refer to it and, for a named parameter, the labels of the calls and the
    /// homonymous parameter of the overrides (with their uses and labels).
    fn local_occurrences(
        &self,
        unit_: UnitId,
        declaration: usize,
        refuse_external: bool,
    ) -> Result<Vec<(UnitId, Span)>, String> {
        let mut output = Vec::new();
        let u = self.program().unit(unit_);
        // `with_declaration`: the declared name is included (in rename, and in the
        // element itself); in references, the homonymous parameters of the
        // hierarchy only contribute their uses (§11.9 E5).
        let uses_of = |unit_: UnitId, declaration: usize, output: &mut Vec<(UnitId, Span)>, with_declaration: bool| {
            let u = self.program().unit(unit_);
            let end = word(&u.source, declaration).map_or(declaration, |s| s.end);
            if with_declaration {
                output.push((
                    unit_,
                    Span {
                        start: declaration,
                        end: end,
                    },
                ));
            }
            let bodies = &self.query.bodies.units[unit_.0 as usize];
            for (e, d) in &bodies.local_declarations {
                if *d == declaration
                    && let ExprKind::Identifier(n) = &u.ast.expr(*e).kind
                {
                    output.push((unit_, n.span));
                }
            }
        };
        uses_of(unit_, declaration, &mut output, true);
        let Some((p, owner)) = parameter_in(&u.ast, declaration) else {
            return Ok(output);
        };
        // Optional positional: in references, length 0 at the start of each passed
        // argument that goes through the index's `visitExpression` (not a simple
        // identifier nor a `MethodInvocation`).
        if p.kind == ast::ParameterKind::Optional && !refuse_external {
            let (function_, positional): (Option<FunctionElementId>, Vec<usize>) = match owner {
                OwnerParameter::Function(fid) => (
                    self.node_function(unit_, fid),
                    u.ast.function(fid).parameters.iter().flatten().filter(|q| q.kind != ast::ParameterKind::Named).filter_map(|q| q.name.map(|n| n.span.start)).collect(),
                ),
                OwnerParameter::Constructor(mid) => (
                    self.node_constructor(unit_, mid),
                    match &u.ast.member(mid).kind {
                        MemberKind::Constructor(k) => k.parameters.iter().filter(|q| q.kind != ast::ParameterKind::Named).filter_map(|q| q.name.map(|n| n.span.start)).collect(),
                        _ => Vec::new(),
                    },
                ),
            };
            if let (Some(f), Some(index)) = (function_, positional.iter().position(|&o| o == declaration)) {
                for (x, s) in self.positional_arguments(f, index) {
                    output.push((x, s));
                }
            }
        }
        if p.kind != ast::ParameterKind::Named || p.public_name.is_some() {
            return Ok(output);
        }
        let Some(label) = p.name.map(|n| n.sym) else {
            return Ok(output);
        };
        let function_ = match owner {
            OwnerParameter::Function(fid) => self.node_function(unit_, fid),
            OwnerParameter::Constructor(mid) => self.node_constructor(unit_, mid),
        };
        let Some(function_) = function_ else {
            return Ok(output);
        };
        // The overrides of an instance method have the same named parameter.
        let fe = self.program().function(function_);
        let mut functions = vec![function_];
        if fe.kind == FunctionKind::Function
            && !fe.static_
            && let Some(c) = fe.class
        {
            let name = self.name(fe.name).to_string();
            let family = self.family(Owner::Class(c), &name, false, refuse_external)?;
            let mut other: Vec<FunctionElementId> =
                family.functions.into_iter().filter(|f| *f != function_).collect();
            other.sort();
            for f in other {
                let FunctionRef::Function { unit, function } = self.program().function(f).node
                else {
                    continue;
                };
                let ast = &self.program().unit(unit).ast;
                let homonym = ast
                    .function(function)
                    .parameters
                    .iter()
                    .flatten()
                    .find(|q| {
                        q.kind == ast::ParameterKind::Named
                            && q.name.is_some_and(|n| n.sym == label)
                    });
                if let Some(n) = homonym.and_then(|q| q.name) {
                    functions.push(f);
                    if self
                        .libraries
                        .contains(&self.program().unit(unit).library)
                    {
                        uses_of(unit, n.span.start, &mut output, refuse_external);
                        // The `[name]` of the homonymous member's doc (the local search has the
                        // whole member as its root), in references and in rename.
                        output.extend(self.docs_that_refer(&Target::Local { unit: unit, declaration: n.span.start }));
                    }
                }
            }
        }
        self.argument_labels(&functions, Some(label), &mut |u, s| output.push((u, s)));
        Ok(output)
    }

    /// The documentation `[ref]`s that resolve to `target`.
    fn docs_that_refer(&self, target: &Target) -> Vec<(UnitId, Span)> {
        self.documentation_references(target, None, |a, _| a == target, false)
    }

    /// The documentation `[ref]`s of the units (of `filter`, when there is one)
    /// whose prefix `r[..=i]` resolves to something `matches` accepts. Each
    /// unit's comments come from the cache ([`Project::unit_comments`]) and only
    /// the prefixes whose last segment is the target's name are resolved (a
    /// prefix only denotes the target by writing its name); the unnamed
    /// constructor, which `[A]` also denotes, resolves everything. With
    /// `index`, the `visitCommentReference` rule: an `[A]`/`[A.b]` that resolves
    /// to a constructor only cites the constructor, and the `[A.name]` of a
    /// constructor goes from the end of the prefix to the end.
    ///
    /// With `index`, a `[x]` in the documentation of a constructor that has a
    /// `this.x` parameter cites the parameter (`ConstructorInitializerScope`),
    /// which the search only includes for the field itself
    /// (`includeParametersForFields`): it counts only when the target is a
    /// member of the field's class.
    fn documentation_references(&self, target: &Target, filter: Option<&HashSet<UnitId>>, matches_: impl Fn(&Target, Option<Concrete>) -> bool, index: bool) -> Vec<(UnitId, Span)> {
        let word = self.target_word(target);
        let mut v = Vec::new();
        for u in self.units_in(filter) {
            let source = &self.program().unit(u).source;
            if !source.contains('[') || word.as_deref().is_some_and(|w| !source.contains(w)) {
                continue;
            }
            for c in self.unit_comments(u).iter() {
                for r in &c.references {
                    if word.as_deref().is_some_and(|w| r.iter().all(|(_, s)| s != w)) {
                        continue;
                    }
                    let only_constructor = index
                        && r.len() <= 2
                        && matches!(self.resolve_doc_reference(u, c.span, r), Some((Target::Constructor(_), _)));
                    for i in 0..r.len() {
                        if only_constructor && i + 1 < r.len() {
                            continue;
                        }
                        if word.as_deref().is_some_and(|w| r[i].1 != w) {
                            continue;
                        }
                        if let Some((a, concrete)) = self.resolve_doc_reference(u, c.span, &r[..=i])
                            && matches_(&a, concrete)
                            && !(index && i == 0 && self.cites_foreign_field_parameter(u, c.span, &r[0].1, target, concrete))
                        {
                            let s = r[i].0;
                            let s = if index && matches!(target, Target::Constructor(_)) && i > 0 { Span { start: r[i - 1].0.end, end: s.end } } else { s };
                            v.push((u, s));
                        }
                    }
                }
            }
        }
        v
    }

    /// The single-name reference `name` of `comment` is a `this.x` parameter of
    /// the documented constructor whose field (`concrete`) is not a member of
    /// `target`'s class: the index records it on the parameter, not the field.
    fn cites_foreign_field_parameter(&self, unit: UnitId, comment: Span, name: &str, target: &Target, concrete: Option<Concrete>) -> bool {
        let Some(Concrete::Variable(v)) = concrete else { return false };
        let ast = &self.program().unit(unit).ast;
        let Some(Documented::Member(m)) = self.documented(unit, comment) else { return false };
        let MemberKind::Constructor(k) = &ast.member(m).kind else { return false };
        let source = &self.program().unit(unit).source;
        let is_field_parameter = k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| &source[n.span.start..n.span.end] == name));
        if !is_field_parameter {
            return false;
        }
        !matches!(target, Target::Member { owner: Owner::Class(c), .. } if self.program().variable(v).class == Some(*c))
    }

    /// The name every occurrence of `target` writes (the identifier's text);
    /// `None` for the constructor, which `[A]`, `super(…)` and the implicit call
    /// cite without its name.
    fn target_word(&self, target: &Target) -> Option<String> {
        let w = match target {
            Target::Local { unit, declaration } | Target::TypeParameter { unit, declaration } => {
                let source = &self.program().unit(*unit).source;
                let s = word(source, *declaration)?;
                source[s.start..s.end].to_string()
            }
            Target::Member { name, .. } => name.clone(),
            Target::Top(el) => self.query.element_name(*el).map(|s| self.name(s).trim_end_matches('=').to_string())?,
            Target::Prefix { name, .. } => self.name(*name).to_string(),
            Target::Constructor(_) => return None,
        };
        let ident = !w.is_empty() && w.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$');
        ident.then_some(w)
    }

    /// The occurrence index of `u` (the top-level element, function or member
    /// variable that each identifier and each access name resolves to), built in
    /// one pass over the expressions and kept until the unit's text changes.
    pub(crate) fn unit_occurrences(&self, u: UnitId) -> std::sync::Arc<UnitOccurrences> {
        let unit = self.program().unit(u);
        let stamp = (unit.source.len(), unit.ast.exprs.len());
        if let Some((c, i)) = self.occurrences_by_unit.lock().ok().and_then(|m| m.get(&u).cloned())
            && c == stamp
        {
            return i;
        }
        let bodies = &self.query.bodies.units[u.0 as usize];
        let mut index = UnitOccurrences::default();
        let mut calls: Option<std::collections::HashMap<ast::ExprId, &ast::Arguments>> = None;
        for (i, e) in unit.ast.exprs.iter().enumerate() {
            let s = match &e.kind {
                ExprKind::Identifier(n) => n.span,
                ExprKind::Property { name, .. } => name.span,
                _ => continue,
            };
            let id = ast::ExprId(i as u32);
            if matches!(e.kind, ExprKind::Property { .. }) && matches!(bodies.get_resolved(id), None | Some(Resolved::Dynamic)) {
                let calls = calls.get_or_insert_with(|| {
                    unit.ast
                        .exprs
                        .iter()
                        .filter_map(|e| match &e.kind {
                            ExprKind::Call { target, arguments } => Some((*target, &**arguments)),
                            _ => None,
                        })
                        .collect()
                });
                if let Some(f) = self.object_member_on_dynamic(u, id, calls.get(&id).copied()) {
                    index.functions.entry(f).or_default().push(s);
                }
                continue;
            }
            match bodies.get_resolved(id) {
                Some(Resolved::Element(r)) => index.elements.entry(*r).or_default().push(s),
                Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                    index.functions.entry(*f).or_default().push(s)
                }
                Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => index.variables.entry(*v).or_default().push(s),
                _ => {}
            }
        }
        // The type annotations (`T`, `p.T`, the `A` of `new A.n()`, the `T` of
        // `new p.T.n()`) and the metadata (`@name`, `@p.name`): the top-level
        // element the name reaches in the library scope (the binding's getter), as
        // in `top_occurrences`.
        let p = self.program();
        let lib = unit.library;
        let mut ty_ = |b: Option<dartforge_elements::model::Binding>, s: Span| {
            if let Some(g) = b.and_then(|b| b.getter) {
                index.type_names.entry(g).or_default().push(s);
            }
        };
        let scopes = type_parameter_scopes(&unit.ast);
        let type_parameter = |n: &ast::Name| scopes.get(&n.sym).is_some_and(|l| l.iter().any(|s| contains(*s, n.span.start)));
        for ty in &unit.ast.types {
            let ast::TypeKind::Named { name, .. } = &ty.kind else { continue };
            match &name[..] {
                [n] if !type_parameter(n) => ty_(p.lookup(lib, n.sym), n.span),
                [pr, n] if is_prefix(p, lib, pr.sym) => ty_(p.lookup_prefixed(lib, pr.sym, n.sym), n.span),
                [c, _] => ty_(p.lookup(lib, c.sym), c.span),
                [pr, n, _] => ty_(p.lookup_prefixed(lib, pr.sym, n.sym), n.span),
                _ => {}
            }
        }
        for a in metadata(&unit.ast, &unit.unit) {
            match &a.name[..] {
                [n] => ty_(p.lookup(lib, n.sym), n.span),
                [n, m, ..] => {
                    ty_(p.lookup(lib, n.sym), n.span);
                    if m.sym != n.sym {
                        ty_(p.lookup_prefixed(lib, n.sym, m.sym), m.span);
                    }
                }
                [] => {}
            }
        }
        let index = std::sync::Arc::new(index);
        if let Ok(mut m) = self.occurrences_by_unit.lock() {
            m.insert(u, (stamp, index.clone()));
        }
        index
    }

    /// The documentation comments of `u` (`dartdoc::comments`), computed once
    /// per unit text.
    pub(crate) fn unit_comments(&self, u: UnitId) -> std::sync::Arc<Vec<dartdoc::Comment>> {
        let source = &self.program().unit(u).source;
        if let Some((n, c)) = self.comments.lock().ok().and_then(|m| m.get(&u).cloned())
            && n == source.len()
        {
            return c;
        }
        let c = std::sync::Arc::new(dartdoc::comments(source));
        if let Ok(mut m) = self.comments.lock() {
            m.insert(u, (source.len(), c.clone()));
        }
        c
    }

    /// The `i`-th positional argument of each call to `f`, when it goes through
    /// the index's `visitExpression`: the empty range at its start.
    pub(crate) fn positional_arguments(&self, f: FunctionElementId, index: usize) -> Vec<(UnitId, Span)> {
        let mut v = Vec::new();
        for u in self.units() {
            let bodies = &self.query.bodies.units[u.0 as usize];
            let ast = &self.program().unit(u).ast;
            for (i, e) in ast.exprs.iter().enumerate() {
                let (arguments_, target_) = match &e.kind {
                    ExprKind::Call { target, arguments } => (&**arguments, Some(*target)),
                    ExprKind::InstanceCreation { arguments, .. } => (&**arguments, None),
                    _ => continue,
                };
                let call = [Some(ast::ExprId(i as u32)), target_].into_iter().flatten().find_map(|x| match bodies.get_resolved(x) {
                    Some(Resolved::Constructor(g))
                    | Some(Resolved::Element(Element::Function(g)))
                    | Some(Resolved::Member { member: MemberRef::Function(g), .. })
                    | Some(Resolved::ExtensionMember { member: g, .. }) => Some(*g),
                    _ => None,
                });
                if call != Some(f) {
                    continue;
                }
                let Some(a) = arguments_.args.iter().filter(|a| a.name.is_none()).nth(index) else { continue };
                let x = ast.expr(a.value);
                let without_visit_expression = match &x.kind {
                    ExprKind::Identifier(_) => true,
                    // `MethodInvocation`: a call with an identifier or property target that is
                    // not a constructor.
                    ExprKind::Call { target, .. } => {
                        matches!(ast.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                            && !matches!(bodies.get_resolved(a.value), Some(Resolved::Constructor(_)))
                    }
                    _ => false,
                };
                if !without_visit_expression {
                    v.push((u, Span { start: x.span.start, end: x.span.start }));
                }
            }
        }
        v
    }

    /// Member: the declarations of the family, the uses resolved to it and, for
    /// a field, the `this.x`s, the initializers and the named labels.
    fn member_occurrences(
        &self,
        owner: Owner,
        name_: &str,
        is_static: bool,
        refuse_external: bool,
        family_parameters: bool,
        filter: Option<&HashSet<UnitId>>,
        by: &mut impl FnMut(UnitId, Span),
    ) -> Result<(), String> {
        let family = self.family(owner, name_, is_static, refuse_external)?;
        for f in &family.functions {
            if let Some((u, s)) = self.function_name(*f)
                && self.libraries.contains(&self.program().unit(u).library)
            {
                by(u, s);
            }
        }
        for v in &family.variables {
            if let Some((u, s)) = self.variable_name(*v)
                && self.libraries.contains(&self.program().unit(u).library)
            {
                by(u, s);
            }
        }
        // The uses in expressions, through each unit's occurrence index.
        for u in self.units_in(filter) {
            let index = self.unit_occurrences(u);
            for f in &family.functions {
                for s in index.functions.get(f).into_iter().flatten() {
                    by(u, *s);
                }
            }
            for v in &family.variables {
                for s in index.variables.get(v).into_iter().flatten() {
                    by(u, *s);
                }
            }
        }
        // `this.x` parameters, `x = e` initializers and the labels of the named
        // `this.x`s in the constructors' calls.
        if !is_static {
            let mut named_constructors = Vec::new();
            let mut symbol = None;
            for c in &family.classes {
                for (u, m) in self.program().class_members(*c) {
                    let MemberKind::Constructor(k) = &self.program().unit(u).ast.member(m).kind
                    else {
                        continue;
                    };
                    for p in k.parameters.iter() {
                        if p.this_
                            && let Some(n) = p.name
                            && self.name(n.sym) == name_
                        {
                            by(u, n.span);
                            symbol = Some(n.sym);
                            if p.kind == ast::ParameterKind::Named
                                && p.public_name.is_none()
                                && let Some(f) = self.node_constructor(u, m)
                            {
                                named_constructors.push(f);
                            }
                        }
                    }
                    for i in k.initializers.iter() {
                        if let ast::Initializer::Field { name, .. } = i
                            && self.name(name.sym) == name_
                        {
                            by(u, name.span);
                        }
                    }
                }
            }
            if !named_constructors.is_empty() {
                self.argument_labels(&named_constructors, symbol, by);
            }
            // The references of the `this.x`s (`includeParametersForFields`): the start
            // of each optional positional argument (length 0, the index's
            // `visitExpression`) and the `super.x`s that point to them
            // (`superConstructorParameter`).
            // References: only the parameters of the pivot field itself
            // (`parameter.field == member` in `getHierarchyMembersAndParameters`),
            // those of the owner's constructors when it declares the field. Rename
            // ([`Project::rename_occurrences`]) renames those of every class of the
            // family.
            let mut parameters: Vec<(FunctionElementId, Option<usize>)> = Vec::new();
            let mut classes: Vec<ClassId> = Vec::new();
            if family_parameters {
                classes = family.classes.clone();
                for v in family.variables.iter() {
                    if let Some(c) = self.program().variable(*v).class
                        && !classes.contains(&c)
                    {
                        classes.push(c);
                    }
                }
            } else if let Owner::Class(c) = owner
                && self.program().class(c).fields.iter().any(|v| {
                    let ve = self.program().variable(*v);
                    !ve.static_ && self.name(ve.name) == name_
                })
            {
                classes.push(c);
            }
            for c in classes {
                for (u, m) in self.program().class_members(c) {
                    let MemberKind::Constructor(k) = &self.program().unit(u).ast.member(m).kind else { continue };
                    let Some(f) = self.node_constructor(u, m) else { continue };
                    let positional: Vec<&ast::Parameter> = k.parameters.iter().filter(|q| q.kind != ast::ParameterKind::Named).collect();
                    for (i, q) in positional.iter().enumerate() {
                        if q.this_ && q.name.is_some_and(|n| self.name(n.sym) == name_) {
                            parameters.push((f, Some(i)));
                            if q.kind == ast::ParameterKind::Optional {
                                for (x, s) in self.positional_arguments(f, i) {
                                    by(x, s);
                                }
                            }
                        }
                    }
                    if k.parameters.iter().any(|q| q.kind == ast::ParameterKind::Named && q.this_ && q.name.is_some_and(|n| self.name(n.sym) == name_)) {
                        parameters.push((f, None));
                    }
                }
            }
            if !parameters.is_empty() {
                for u in self.units_in(filter) {
                    let ast = &self.program().unit(u).ast;
                    for (mi, m) in ast.members.iter().enumerate() {
                        let MemberKind::Constructor(k) = &m.kind else { continue };
                        if !k.parameters.iter().any(|q| q.super_) {
                            continue;
                        }
                        let Some(sub) = self.node_constructor(u, ast::MemberId(mi as u32)) else { continue };
                        let Some(sup) = self.super_constructor(sub) else { continue };
                        let mut position = 0usize;
                        for q in k.parameters.iter() {
                            let named = q.kind == ast::ParameterKind::Named;
                            if q.super_ {
                                let target_q = if named { None } else { Some(position) };
                                let matches = parameters.iter().any(|&(f, i)| {
                                    f == sup
                                        && match (i, target_q) {
                                            (Some(a), Some(b)) => a == b,
                                            (None, None) => q.name.is_some_and(|n| self.name(n.sym) == name_),
                                            _ => false,
                                        }
                                });
                                if matches && let Some(n) = q.name {
                                    by(u, n.span);
                                }
                                if !named {
                                    position += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Named constructor: the declaration, the creations (`A.name()`,
    /// `new A.name()`), the tear-offs, the redirections (`this.name()`,
    /// `super.name()`, `= A.name`), the enum constants and the metadata.
    fn constructor_occurrences(&self, f: FunctionElementId, filter: Option<&HashSet<UnitId>>, by: &mut impl FnMut(UnitId, Span)) {
        // The index relations (§11.1.2): a named constructor is `.name` (with the
        // dot; `A.new` gives `.new`); the unnamed one has length 0 at the end of
        // the type, of `super`/`this`, of the enum constant's name; the implicit
        // call to the super constructor is the `A`/`A.name` of the child
        // constructor, or the name of the class that only has the synthetic one.
        let p = self.program();
        if let Some((u, s)) = self.function_name(f) {
            by(u, s);
        }
        let Some(class) = p.function(f).class else {
            return;
        };
        let symbol = p.function(f).name;
        let unnamed = self.name(symbol).is_empty();
        let empty = |o: usize| Span { start: o, end: o };
        for u in self.units_in(filter) {
            let unit = p.unit(u);
            let ast = &unit.ast;
            let source = unit.source.as_str();
            let with_dot = |s: Span| {
                let before = source[..s.start].trim_end();
                if before.ends_with('.') { Span { start: before.len() - 1, end: s.end } } else { s }
            };
            let bodies = &self.query.bodies.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                let id = ast::ExprId(i as u32);
                let this = matches!(bodies.get_resolved(id), Some(Resolved::Constructor(g)) if *g == f)
                    || bodies.get_resolved(id).is_none() && call_constructor(ast, bodies, id) == Some(f)
                    || matches!(bodies.get_resolved(id), Some(Resolved::Element(Element::Class(c))) if *c == class)
                        && call_constructor(ast, bodies, id) == Some(f);
                if !this {
                    continue;
                }
                match &e.kind {
                    ExprKind::Property { name, .. } if name.sym == symbol || (unnamed && self.name(name.sym) == "new") => by(u, with_dot(name.span)),
                    // `p.A()`: the prefixed type.
                    ExprKind::Property { .. } if unnamed => by(u, empty(e.span.end)),
                    ExprKind::Identifier(_) | ExprKind::TypeArguments { .. } if unnamed => by(u, empty(e.span.end)),
                    ExprKind::InstanceCreation { constructor: Some(n), .. } if n.sym == symbol || (unnamed && self.name(n.sym) == "new") => {
                        by(u, with_dot(n.span))
                    }
                    // `new A.name()`: the constructor name comes in the type.
                    ExprKind::InstanceCreation { constructor: None, ty, .. } => {
                        if unnamed {
                            if self.constructor_in_type(u, *ty, class, symbol).is_none() {
                                by(u, empty(ast.ty(*ty).span.end));
                            }
                        } else if let Some(n) = self.constructor_in_type(u, *ty, class, symbol) {
                            by(u, with_dot(n));
                        }
                    }
                    _ => {}
                }
            }
            for (mi, m) in ast.members.iter().enumerate() {
                let MemberKind::Constructor(k) = &m.kind else {
                    continue;
                };
                // The class that owns this constructor, for `this.`/`super.`.
                let owner = self.node_constructor(u, ast::MemberId(mi as u32)).and_then(|g| p.function(g).class);
                let mut calls_super = false;
                for start in k.initializers.iter() {
                    let (target_class, name_, span, word) = match start {
                        ast::Initializer::Redirect { constructor, span, .. } => {
                            calls_super = true;
                            (owner, *constructor, *span, "this")
                        }
                        ast::Initializer::Super { constructor, span, .. } => {
                            calls_super = true;
                            (owner.and_then(|d| p.class(d).supertype_class), *constructor, *span, "super")
                        }
                        _ => continue,
                    };
                    if target_class != Some(class) {
                        continue;
                    }
                    match name_ {
                        Some(n) if n.sym == symbol => by(u, with_dot(n.span)),
                        None if unnamed => by(u, empty(span.start + word.len())),
                        _ => {}
                    }
                }
                if let Some(r) = &k.redirect {
                    calls_super = true;
                    match r.constructor {
                        Some(n) if n.sym == symbol && self.type_class(u, r.ty) == Some(class) => by(u, with_dot(n.span)),
                        None => {
                            if let Some(n) = self.constructor_in_type(u, r.ty, class, symbol) {
                                by(u, with_dot(n));
                            } else if unnamed && self.type_class(u, r.ty) == Some(class) {
                                by(u, empty(ast.ty(r.ty).span.end));
                            }
                        }
                        _ => {}
                    }
                }
                // The implicit call to the unnamed super constructor.
                if unnamed
                    && !calls_super
                    && !k.factory
                    && let Some(d) = owner
                    && p.class(d).supertype_class == Some(class)
                {
                    let end = k.name.map_or(k.class_name.span.end, |n| n.span.end);
                    by(u, Span { start: k.class_name.span.start, end: end });
                }
            }
            // A class that only has the synthetic constructor calls the superclass's
            // unnamed one: its name.
            if unnamed {
                for (ci, c) in p.classes.iter().enumerate() {
                    let Some(dr) = c.decl else { continue };
                    if dr.unit != u || c.supertype_class != Some(class) {
                        continue;
                    }
                    let DeclKind::Class(cd) = &ast.decl(dr.decl).kind else { continue };
                    if cd.mixin_application {
                        continue;
                    }
                    let synthetic_only = c.constructors.values().all(|g| p.function(*g).kind == FunctionKind::SyntheticConstructor);
                    let _ = ci;
                    if synthetic_only {
                        by(u, cd.name.span);
                    }
                }
            }
            // Enum constants: `a.name(...)` is `.name`; without a selector, the unnamed
            // constructor at the end of the name.
            for d in &ast.decls {
                if let DeclKind::Enum(e) = &d.kind {
                    let is_owner = p.class(class).decl.is_some_and(|dr| dr.unit == u && std::ptr::eq(ast.decl(dr.decl), d));
                    if !is_owner {
                        continue;
                    }
                    for c in &e.constants {
                        match c.constructor {
                            Some(n) if n.sym == symbol => by(u, with_dot(n.span)),
                            None if unnamed => by(u, empty(c.name.span.end)),
                            _ => {}
                        }
                    }
                }
            }
            // Metadata `@A.name(...)` and `@p.A.name(...)`: the `name` (without the
            // dot, a `SimpleIdentifier`).
            for a in metadata(ast, &unit.unit) {
                let parts: Vec<SymbolId> = a.name.iter().map(|n| n.sym).collect();
                if parts.len() >= 2
                    && let Some((Target::Constructor(g), _)) = self.resolve_path(unit.library, None, &parts)
                    && g == f
                {
                    by(u, a.name[a.name.len() - 1].span);
                }
            }
        }
    }

    /// `A.name` written as a type (`new A.name()`, `= A.name`) when `A` is
    /// `class` (and not a prefix) and `name` is `symbol`: the span of `name`.
    fn constructor_in_type(
        &self,
        u: UnitId,
        ty: ast::TypeId,
        class: ClassId,
        symbol: SymbolId,
    ) -> Option<Span> {
        let p = self.program();
        let unit = p.unit(u);
        let ast::TypeKind::Named { name, .. } = &unit.ast.ty(ty).kind else {
            return None;
        };
        let [a, n] = &name[..] else { return None };
        if n.sym != symbol || is_prefix(p, unit.library, a.sym) {
            return None;
        }
        match p.lookup(unit.library, a.sym)?.getter? {
            Element::Class(c) if c == class => Some(n.span),
            _ => None,
        }
    }

    /// The class of a type annotation `A` or `p.A` written in `u`.
    pub(crate) fn type_class(&self, u: UnitId, ty: ast::TypeId) -> Option<ClassId> {
        let p = self.program();
        let unit = p.unit(u);
        let ast::TypeKind::Named { name, .. } = &unit.ast.ty(ty).kind else {
            return None;
        };
        let binding = match &name[..] {
            [n] => p.lookup(unit.library, n.sym),
            [pr, n] => p.lookup_prefixed(unit.library, pr.sym, n.sym),
            _ => None,
        }?;
        match binding.getter? {
            Element::Class(c) => Some(c),
            _ => None,
        }
    }

    /// `references` on an import prefix (§11.5): in a type (`p.A`) it is the
    /// `PrefixElement` (each `p` of the library's expressions and types, without
    /// the `as p`; declaration = the name in the `as p`); in an expression or on
    /// the `as p` it is the `LibraryImportElement` chosen by `getImportElement`
    /// (each `p.` with the dot of the uses of elements of its namespace;
    /// declaration = `(start of the import, 0)`). `None` without an import.
    pub(crate) fn prefix_references(
        &self,
        unit: UnitId,
        offset: usize,
        lib: LibraryId,
        name: SymbolId,
    ) -> Option<(Option<(UnitId, Span)>, Vec<(UnitId, Span)>)> {
        self.prefix_references_with(unit, offset, lib, name, false)
    }

    /// The `LibraryImportElement` that `getElementOfNode` gives for the prefix
    /// used in an expression at `offset` (`getImportElement`): its index in the
    /// library's `imports`. `None` in a type (`p.T`, the `PrefixElement`) or when
    /// the used element does not resolve.
    pub(crate) fn prefix_import(&self, unit: UnitId, offset: usize, lib: LibraryId, name: SymbolId) -> Option<usize> {
        let (declaration, _) = self.prefix_references_with(unit, offset, lib, name, true)?;
        let (_, s) = declaration?;
        (s.start == s.end).then_some(s.start)
    }

    fn prefix_references_with(
        &self,
        unit: UnitId,
        offset: usize,
        lib: LibraryId,
        name_: SymbolId,
        only_import: bool,
    ) -> Option<(Option<(UnitId, Span)>, Vec<(UnitId, Span)>)> {
        let p = self.program();
        let u = p.unit(unit);
        let ast = &u.ast;
        let in_type = ast.types.iter().any(|t| match &t.kind {
            ast::TypeKind::Named { name, .. } => matches!(&name[..], [pr, _] if pr.span.start <= offset && offset <= pr.span.end),
            _ => false,
        });
        if in_type {
            if only_import {
                return None;
            }
            let mut uses = Vec::new();
            for &x in &p.library(lib).units {
                let un = p.unit(x);
                let bodies = &self.query.bodies.units[x.0 as usize];
                for (i, e) in un.ast.exprs.iter().enumerate() {
                    if let ExprKind::Identifier(n) = &e.kind
                        && n.sym == name_
                        && matches!(
                            bodies.get_resolved(ast::ExprId(i as u32)),
                            Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))
                        )
                    {
                        uses.push((x, n.span));
                    }
                }
                for t in &un.ast.types {
                    if let ast::TypeKind::Named { name, .. } = &t.kind
                        && let [pr, _] = &name[..]
                        && pr.sym == name_
                    {
                        uses.push((x, pr.span));
                    }
                }
            }
            let declaration = p.library(lib).imports.iter().find_map(|i| {
                if i.prefix != Some(name_) {
                    return None;
                }
                match &p.unit(i.unit).unit.directives.get(i.directive)?.kind {
                    ast::DirectiveKind::Import { prefix: Some(n), .. } => Some((i.unit, n.span)),
                    _ => None,
                }
            });
            return Some((declaration, uses));
        }
        // The import: that of the `as p` under the cursor, or `getImportElement`
        // through the element used after `p.`.
        let defining = *p.library(lib).units.first()?;
        let imports: Vec<&dartforge_elements::model::Import> =
            p.library(lib).imports.iter().filter(|i| i.unit == defining && i.prefix == Some(name_)).collect();
        let node_as = imports.iter().position(|i| match p.unit(i.unit).unit.directives.get(i.directive).map(|d| &d.kind) {
            Some(ast::DirectiveKind::Import { prefix: Some(n), .. }) => i.unit == unit && n.span.start <= offset && offset <= n.span.end,
            _ => false,
        });
        let chosen = match node_as {
            Some(k) => Some(k),
            None => {
                let bodies = &self.query.bodies.units[unit.0 as usize];
                let used = ast.exprs.iter().enumerate().find_map(|(i, e)| match &e.kind {
                    ExprKind::Property { target, name, .. } => match &ast.expr(*target).kind {
                        ExprKind::Identifier(pn) if pn.sym == name_ && pn.span.start <= offset && offset <= pn.span.end => {
                            let id = ast::ExprId(i as u32);
                            let r = bodies.get_resolved(id).cloned().or_else(|| call_constructor(ast, bodies, id).map(Resolved::Constructor));
                            Some((r, name.sym))
                        }
                        _ => None,
                    },
                    _ => None,
                });
                let (resolved, used_name) = used?;
                let element = match resolved {
                    Some(Resolved::Element(el)) => el,
                    Some(Resolved::Constructor(f)) => Element::Class(p.function(f).class?),
                    _ => return None,
                };
                let element_lib = self.element_library(element);
                let candidates: Vec<usize> = (0..imports.len()).filter(|&k| imports[k].library == element_lib).collect();
                if let Some(&k) = candidates.iter().find(|&&k| imports[k].combinators.is_empty()) {
                    Some(k)
                } else if candidates.len() == 1 {
                    Some(candidates[0])
                } else {
                    candidates
                        .into_iter()
                        .find(|&k| p.library(imports[k].library).exported.get(&used_name).and_then(|b| b.getter.or(b.setter)) == Some(element))
                }
            }
        };
        let imp = imports.get(chosen?)?;
        let directive = p.unit(imp.unit).unit.directives.get(imp.directive)?;
        let declaration = Some((imp.unit, Span { start: directive.span.start, end: directive.span.start }));
        if only_import {
            let index = p.library(lib).imports.iter().position(|i| std::ptr::eq(i, *imp))?;
            return Some((Some((imp.unit, Span { start: index, end: index })), Vec::new()));
        }
        // The `p.` uses of elements of this import's namespace.
        let namespace = &p.library(imp.library).exported;
        let visible = |s: SymbolId| {
            namespace.contains_key(&s)
                && imp.combinators.iter().all(|c| match c {
                    ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == s),
                    ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == s),
                })
        };
        let mut uses = Vec::new();
        for &x in &p.library(lib).units {
            let un = p.unit(x);
            let fx = un.source.as_str();
            let until_next = |pr: Span| {
                let after_dot = fx[pr.end..].find('.').map_or(pr.end, |k| pr.end + k + 1);
                let spaces = fx[after_dot..].len() - fx[after_dot..].trim_start().len();
                Span { start: pr.start, end: after_dot + spaces }
            };
            for e in un.ast.exprs.iter() {
                if let ExprKind::Property { target, name, .. } = &e.kind
                    && let ExprKind::Identifier(pn) = &un.ast.expr(*target).kind
                    && pn.sym == name_
                    && is_prefix(p, lib, name_)
                    && visible(name.sym)
                {
                    uses.push((x, until_next(pn.span)));
                }
            }
            for t in &un.ast.types {
                if let ast::TypeKind::Named { name, .. } = &t.kind
                    && let [pr, n] = &name[..]
                    && pr.sym == name_
                    && visible(n.sym)
                {
                    uses.push((x, until_next(pr.span)));
                }
            }
            // `[p.x]` in the documentation: the index also cites the import.
            if fx.contains('[') {
                let prefix_text = self.name(name_);
                for c in dartdoc::comments(fx) {
                    for r in &c.references {
                        if r.len() >= 2
                            && r[0].1 == prefix_text
                            && self.query.names.lookup(&r[1].1).is_some_and(visible)
                        {
                            uses.push((x, until_next(r[0].0)));
                        }
                    }
                }
            }
        }
        uses.sort_by_key(|(u, s)| (*u, s.start));
        uses.dedup();
        Some((declaration, uses))
    }

    /// Import prefix: the `as p` of each import of the library and each `p.`
    /// written in it (expressions, types, metadata).
    fn prefix_occurrences(
        &self,
        lib: LibraryId,
        name_: SymbolId,
        by: &mut impl FnMut(UnitId, Span),
    ) {
        let p = self.program();
        for i in &p.library(lib).imports {
            if i.prefix == Some(name_)
                && let Some(d) = p.unit(i.unit).unit.directives.get(i.directive)
                && let ast::DirectiveKind::Import {
                    prefix: Some(n), ..
                } = &d.kind
            {
                by(i.unit, n.span);
            }
        }
        if !self.libraries.contains(&lib) {
            return;
        }
        for &u in &p.library(lib).units {
            let unit = p.unit(u);
            let ast = &unit.ast;
            let bodies = &self.query.bodies.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                if let ExprKind::Identifier(n) = &e.kind
                    && n.sym == name_
                    && matches!(
                        bodies.get_resolved(ast::ExprId(i as u32)),
                        Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))
                    )
                {
                    by(u, n.span);
                }
            }
            for t in &ast.types {
                if let ast::TypeKind::Named { name, .. } = &t.kind
                    && let [pr, _] = &name[..]
                    && pr.sym == name_
                {
                    by(u, pr.span);
                }
            }
            for a in metadata(ast, &unit.unit) {
                if a.name.len() >= 2 && a.name[0].sym == name_ && is_prefix(p, lib, name_) {
                    by(u, a.name[0].span);
                }
            }
        }
    }

    /// Type parameter: the declaration and the uses (types and expressions)
    /// whose innermost parameter with that name is it.
    fn type_parameter_occurrences(
        &self,
        unit: UnitId,
        declaration: usize,
        by: &mut impl FnMut(UnitId, Span),
    ) {
        let u = self.program().unit(unit);
        let ast = &u.ast;
        let Some(name_) = word(&u.source, declaration) else {
            return;
        };
        by(unit, name_);
        let text = &u.source[name_.start..name_.end];
        for t in &ast.types {
            if let ast::TypeKind::Named { name, .. } = &t.kind
                && let [n] = &name[..]
                && self.name(n.sym) == text
                && type_parameter_declaration(ast, n.span.start, n.sym) == Some(declaration)
            {
                by(unit, n.span);
            }
        }
        let bodies = &self.query.bodies.units[unit.0 as usize];
        for (i, e) in ast.exprs.iter().enumerate() {
            if let ExprKind::Identifier(n) = &e.kind
                && self.name(n.sym) == text
                && matches!(
                    bodies.get_resolved(ast::ExprId(i as u32)),
                    Some(Resolved::TypeParameter(_))
                )
                && type_parameter_declaration(ast, n.span.start, n.sym) == Some(declaration)
            {
                by(unit, n.span);
            }
        }
    }

    /// `name:` labels of the arguments in the calls of `functions`.
    pub(crate) fn argument_labels(
        &self,
        functions: &[FunctionElementId],
        label: Option<SymbolId>,
        by: &mut impl FnMut(UnitId, Span),
    ) {
        let Some(label) = label else { return };
        for u in self.units() {
            let bodies = &self.query.bodies.units[u.0 as usize];
            let ast = &self.program().unit(u).ast;
            for (i, e) in ast.exprs.iter().enumerate() {
                let (arguments_, target_) = match &e.kind {
                    ExprKind::Call { target, arguments } => (&**arguments, Some(*target)),
                    ExprKind::InstanceCreation { arguments, .. } => (&**arguments, None),
                    _ => continue,
                };
                let call = [Some(ast::ExprId(i as u32)), target_]
                    .into_iter()
                    .flatten()
                    .find_map(|x| match bodies.get_resolved(x) {
                        Some(Resolved::Constructor(f))
                        | Some(Resolved::Element(Element::Function(f)))
                        | Some(Resolved::Member {
                            member: MemberRef::Function(f),
                            ..
                        })
                        | Some(Resolved::ExtensionMember { member: f, .. }) => Some(*f),
                        _ => None,
                    });
                if !call.is_some_and(|f| functions.contains(&f)) {
                    continue;
                }
                for a in arguments_.args.iter() {
                    if let Some(n) = a.name
                        && n.sym == label
                    {
                        by(u, n.span);
                    }
                }
            }
        }
    }

    /// Elements a top-level target covers: homonymous getter and setter, the
    /// variable and its accessors.
    pub(crate) fn top_elements(&self, el: Element) -> Vec<Element> {
        let p = self.program();
        let mut v = vec![el];
        let (lib, name) = match el {
            Element::Variable(x) => (p.variable(x).library, p.variable(x).name),
            Element::Function(f) => (p.function(f).library, p.function(f).name),
            _ => return v,
        };
        if let Some(b) = p.library(lib).declared.get(&name) {
            v.extend(b.getter);
            v.extend(b.setter);
        }
        for e in v.clone() {
            if let Element::Variable(x) = e {
                v.extend(p.variable(x).getter.map(Element::Function));
                v.extend(p.variable(x).setter.map(Element::Function));
            }
        }
        v.sort_by_key(|e| format!("{e:?}"));
        v.dedup();
        v
    }

    /// Top level: declarations, uses, types, metadata and `show`/`hide`.
    fn top_occurrences(&self, el: Element, filter: Option<&HashSet<UnitId>>, by: &mut impl FnMut(UnitId, Span)) {
        let p = self.program();
        let elements = self.top_elements(el);
        let Some(symbol) = self.query.element_name(el) else {
            return;
        };
        // Declarations.
        for e in &elements {
            if let Some((u, s)) = self.top_element_name(*e) {
                by(u, s);
            }
            // Constructors written with the class name.
            if let Element::Class(c) = e {
                for (u, m) in p.class_members(*c) {
                    if let MemberKind::Constructor(k) = &p.unit(u).ast.member(m).kind
                        && k.class_name.sym == symbol
                    {
                        by(u, k.class_name.span);
                    }
                }
            }
        }
        for u in self.units_in(filter) {
            // Uses in expressions, through the unit's occurrence index.
            let index = self.unit_occurrences(u);
            for el in &elements {
                for s in index.elements.get(el).into_iter().flatten() {
                    by(u, *s);
                }
            }
            // Type annotations and metadata, through the same index.
            for el in &elements {
                for s in index.type_names.get(el).into_iter().flatten() {
                    by(u, *s);
                }
            }
        }
        // `show`/`hide` of imports and exports that reach the element.
        for lib in &self.libraries {
            let l = p.library(*lib);
            let directives = l
                .imports
                .iter()
                .map(|i| (i.unit, i.directive, i.library))
                .chain(l.exports.iter().map(|e| (e.unit, e.directive, e.library)));
            for (u, d, target) in directives {
                let Some(dir) = p.unit(u).unit.directives.get(d) else {
                    continue;
                };
                let combinators_ = match &dir.kind {
                    ast::DirectiveKind::Import { combinators, .. }
                    | ast::DirectiveKind::Export { combinators, .. } => combinators,
                    _ => continue,
                };
                let exports = p
                    .library(target)
                    .exported
                    .get(&symbol)
                    .and_then(|b| b.getter.or(b.setter))
                    .is_some_and(|g| elements.contains(&g));
                if !exports {
                    continue;
                }
                for c in combinators_ {
                    let (ast::Combinator::Show(names) | ast::Combinator::Hide(names)) = c;
                    for n in names {
                        if n.sym == symbol {
                            by(u, n.span);
                        }
                    }
                }
            }
        }
    }

    /// Superclasses, mixins, interfaces and `on` of `c`, transitively.
    pub(crate) fn supertypes(&self, c: ClassId) -> HashSet<ClassId> {
        let mut seen = HashSet::new();
        let mut stack = vec![c];
        while let Some(x) = stack.pop() {
            let cl = self.program().class(x);
            for s in cl
                .supertype_class
                .iter()
                .chain(&cl.mixin_classes)
                .chain(&cl.interface_classes)
                .chain(&cl.on_classes)
            {
                if seen.insert(*s) {
                    stack.push(*s);
                }
            }
        }
        seen
    }

    /// The direct subtypes of each class through the **written** supertypes
    /// (`_addSubtype`, index.dart:1047-1096): `extends`/`with`/`implements` of a
    /// class and of a class alias, `on`/`implements` of a mixin,
    /// `with`/`implements` of an enum and `implements` of an extension type; a
    /// supertype written through a `typedef` and the implicit `Object` do not
    /// count.
    pub(crate) fn written_subtypes(&self) -> std::sync::Arc<WrittenSubtypes> {
        if let Some(m) = self.written_subtypes.lock().ok().and_then(|m| m.clone()) {
            return m;
        }
        let m = std::sync::Arc::new(self.compute_written_subtypes());
        if let Ok(mut c) = self.written_subtypes.lock() {
            *c = Some(m.clone());
        }
        m
    }

    fn compute_written_subtypes(&self) -> WrittenSubtypes {
        let p = self.program();
        let mut m: std::collections::HashMap<ClassId, Vec<ClassId>> = std::collections::HashMap::new();
        // The index's `visitClassDeclaration`: a class without `extends` has the
        // implicit `IS_EXTENDED_BY` from `Object`, which the subtype search
        // (`Search._addResults`) finds in the files that cite the name `Object`
        // and in those of its library.
        let object = self.query.core.object_class;
        let mut eligible: std::collections::HashMap<UnitId, bool> = std::collections::HashMap::new();
        for (i, c) in p.classes.iter().enumerate() {
            let Some(d) = c.decl else { continue };
            let unit = p.unit(d.unit);
            let ast = &unit.ast;
            if let (Some(o), DeclKind::Class(x)) = (object, &ast.decl(d.decl).kind)
                && x.extends.is_none()
                && !x.mixin_application
                && ClassId(i as u32) != o
                && *eligible.entry(d.unit).or_insert_with(|| {
                    unit.library == p.class(o).library
                        || (unit.role != dartforge_elements::model::UnitRole::Patch
                            && unit.source.contains("Object")
                            && self.unit_referenced_names(d.unit).contains("Object"))
                })
            {
                let v = m.entry(o).or_default();
                if !v.contains(&ClassId(i as u32)) {
                    v.push(ClassId(i as u32));
                }
            }
            let types: Vec<ast::TypeId> = match &ast.decl(d.decl).kind {
                DeclKind::Class(x) => x.extends.iter().chain(x.with.iter()).chain(x.implements.iter()).copied().collect(),
                DeclKind::Mixin(x) => x.on.iter().chain(x.implements.iter()).copied().collect(),
                DeclKind::Enum(x) => x.with.iter().chain(x.implements.iter()).copied().collect(),
                DeclKind::ExtensionType(x) => x.implements.to_vec(),
                _ => Vec::new(),
            };
            for t in types {
                let ast::TypeKind::Named { name, .. } = &ast.ty(t).kind else { continue };
                let binding = match &name[..] {
                    [n] => p.lookup_in_unit(d.unit, n.sym),
                    [pf, n] => p.lookup_prefixed_in_unit(d.unit, pf.sym, n.sym),
                    _ => None,
                };
                if let Some(Element::Class(s)) = binding.and_then(|b| b.getter) {
                    let v = m.entry(s).or_default();
                    let sub = ClassId(i as u32);
                    if !v.contains(&sub) {
                        v.push(sub);
                    }
                }
            }
        }
        m
    }

    /// Members of `c` with base name `name` (instance or static).
    pub(crate) fn declared(
        &self,
        c: ClassId,
        name: &str,
        is_static: bool,
    ) -> Vec<FunctionElementId> {
        let cl = self.program().class(c);
        let map = if is_static {
            &cl.static_members
        } else {
            &cl.instance_members
        };
        let mut v: Vec<FunctionElementId> = map
            .iter()
            .filter(|(s, _)| base_name(self.name(**s)) == name)
            .map(|(_, f)| *f)
            .collect();
        v.sort();
        v
    }

    /// An error when `lib` does not belong to the project (rename only touches
    /// the project).
    fn refuse_external_one(&self, lib: LibraryId, name: &str) -> Result<(), String> {
        if self.in_project(lib) {
            return Ok(());
        }
        let uri = &self.program().library(lib).uri;
        Err(format!(
            "'{name}' is declared in {uri}, outside the project; only project declarations can be renamed."
        ))
    }

    /// The family of a member: functions, variables and classes. Instance
    /// members walk up and down the hierarchy until it closes; statics and
    /// extension members are only the owner's.
    ///
    /// # Errors
    ///
    /// With `refuse_external`, when some member of the family is declared
    /// outside the project (SDK or package).
    pub(crate) fn family(
        &self,
        owner: Owner,
        name: &str,
        is_static: bool,
        refuse_external: bool,
    ) -> Result<Family, String> {
        let p = self.program();
        let refuse = |lib: LibraryId| {
            if refuse_external {
                self.refuse_external_one(lib, name)
            } else {
                Ok(())
            }
        };
        let mut functions = HashSet::new();
        let mut variables = HashSet::new();
        let classes: Vec<ClassId> = match owner {
            Owner::Extension(x) => {
                let ext = p.extension(x);
                refuse(ext.library)?;
                let map = if is_static {
                    &ext.static_members
                } else {
                    &ext.instance_members
                };
                for (s, f) in map {
                    if base_name(self.name(*s)) == name {
                        functions.insert(*f);
                    }
                }
                for v in &ext.fields {
                    if self.name(p.variable(*v).name) == name {
                        variables.insert(*v);
                    }
                }
                Vec::new()
            }
            Owner::Class(c) if is_static => {
                refuse(p.class(c).library)?;
                functions.extend(self.declared(c, name, true));
                for v in p.class(c).fields.iter().chain(&p.class(c).enum_constants) {
                    let ve = p.variable(*v);
                    if ve.static_ && self.name(ve.name) == name {
                        variables.insert(*v);
                    }
                }
                vec![c]
            }
            Owner::Class(c) => {
                // `getHierarchyMembersAndParameters` (hierarchy.dart:106-181): one walk up
                // the owner's supertypes and one walk down the written subtypes of each one
                // that declares the name.
                let library = p.class(c).library;
                let private = name.starts_with('_');
                let declares = |x: ClassId| !self.declared(x, name, false).is_empty() || !self.declared(x, name, true).is_empty();
                let mut supers: Vec<ClassId> = self
                    .supertypes(c)
                    .into_iter()
                    .filter(|&s| !private || p.class(s).library == library)
                    .collect();
                supers.sort_by_key(|s| s.0);
                let direct = self.written_subtypes();
                let mut descent: Vec<ClassId> = Vec::new();
                for x in supers.into_iter().chain([c]) {
                    if !declares(x) {
                        continue;
                    }
                    // `appendAllSubtypes`: DFS through the direct subtypes.
                    let mut stack = vec![x];
                    while let Some(y) = stack.pop() {
                        for &s in direct.get(&y).map(|v| &v[..]).unwrap_or(&[]) {
                            if !descent.contains(&s) {
                                descent.push(s);
                                stack.push(s);
                            }
                        }
                    }
                    if !descent.contains(&x) {
                        descent.push(x);
                    }
                }
                if private {
                    descent.retain(|&s| p.class(s).library == library);
                }
                for x in &descent {
                    let doses: Vec<FunctionElementId> = self.declared(*x, name, false).into_iter().chain(self.declared(*x, name, true)).collect();
                    if doses.is_empty() {
                        continue;
                    }
                    refuse(p.class(*x).library)?;
                    for f in doses {
                        functions.insert(f);
                        if let Some(v) = p.function(f).variable {
                            variables.insert(v);
                        }
                    }
                }
                descent
            }
        };
        for v in variables.clone() {
            functions.extend(p.variable(v).getter);
            functions.extend(p.variable(v).setter);
        }
        Ok(Family {
            functions,
            variables,
            classes,
        })
    }
}

/// Functions, variables and classes of a member family.
pub(crate) struct Family {
    pub functions: HashSet<FunctionElementId>,
    pub variables: HashSet<VariableId>,
    pub classes: Vec<ClassId>,
}

/// The declaration that a documentation comment documents.
#[derive(Debug, Clone, Copy)]
enum Documented {
    Decl(ast::DeclId),
    Member(ast::MemberId),
}

/// The constructor called when `target` is the target of a call resolved to
/// a constructor (`A(…)`, `A.name(…)`, `p.A.name(…)` without `new`).
fn call_constructor(
    ast: &Ast,
    bodies: &dartforge_types::UnitBodyTypes,
    target_: ast::ExprId,
) -> Option<FunctionElementId> {
    ast.exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Call { target, .. } if *target == target_ => {
                match bodies.get_resolved(ast::ExprId(i as u32)) {
                    Some(Resolved::Constructor(f)) => Some(*f),
                    _ => None,
                }
            }
            _ => None,
        })
}

/// `name` is an import prefix in `lib` (and not a name of the scope).
pub(crate) fn is_prefix(p: &Program, lib: LibraryId, name: SymbolId) -> bool {
    p.library(lib).prefixes.contains_key(&name)
        && p.lookup(lib, name)
            .is_none_or(|b| matches!(b.getter, None | Some(Element::Prefix(..))))
}

/// All the metadata annotations of the unit (declarations, members,
/// directives, parameters, enum constants and type parameters).
pub(crate) fn metadata<'a>(
    ast: &'a Ast,
    unit: &'a ast::CompilationUnit,
) -> Vec<&'a ast::Annotation> {
    let mut v: Vec<&ast::Annotation> = Vec::new();
    v.extend(unit.directives.iter().flat_map(|d| d.metadata.iter()));
    v.extend(ast.decls.iter().flat_map(|d| d.metadata.iter()));
    v.extend(ast.members.iter().flat_map(|m| m.metadata.iter()));
    for d in &ast.decls {
        if let DeclKind::Enum(e) = &d.kind {
            v.extend(e.constants.iter().flat_map(|c| c.metadata.iter()));
        }
    }
    for f in &ast.functions {
        v.extend(
            f.parameters
                .iter()
                .flatten()
                .flat_map(|p| p.metadata.iter()),
        );
    }
    for m in &ast.members {
        if let MemberKind::Constructor(k) = &m.kind {
            v.extend(k.parameters.iter().flat_map(|p| p.metadata.iter()));
        }
    }
    v
}

/// Where a parameter was declared.
#[derive(Clone, Copy)]
pub(crate) enum OwnerParameter {
    Function(ast::FunctionId),
    Constructor(ast::MemberId),
}

/// The parameter whose name starts at `offset`.
pub(crate) fn parameter_in(ast: &Ast, offset: usize) -> Option<(&Parameter, OwnerParameter)> {
    for (i, f) in ast.functions.iter().enumerate() {
        if let Some(ps) = &f.parameters
            && let Some(p) = ps
                .iter()
                .find(|p| p.name.is_some_and(|n| n.span.start == offset))
        {
            return Some((p, OwnerParameter::Function(ast::FunctionId(i as u32))));
        }
    }
    for (i, m) in ast.members.iter().enumerate() {
        if let MemberKind::Constructor(k) = &m.kind
            && let Some(p) = k
                .parameters
                .iter()
                .find(|p| p.name.is_some_and(|n| n.span.start == offset))
        {
            return Some((p, OwnerParameter::Constructor(ast::MemberId(i as u32))));
        }
    }
    None
}

/// `name` declares the variable of a `for (… x in …)` (statement or
/// collection element): the analyzer's `DeclaredIdentifier`, whose name has
/// no navigation region.
fn for_in_variable(ast: &Ast, name: Span) -> bool {
    let declared = |t: &ast::ForInTarget| matches!(t, ast::ForInTarget::Declared { name: n, .. } if n.span == name);
    ast.stmts.iter().any(|s| matches!(&s.kind, ast::StmtKind::ForIn { target, .. } if declared(target)))
        || ast.exprs.iter().any(|e| collection_elements(&e.kind).iter().any(|c| for_in_element(c, &declared)))
}

fn collection_elements(kind: &ExprKind) -> &[ast::CollectionElement] {
    match kind {
        ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements,
        _ => &[],
    }
}

fn for_in_element(c: &ast::CollectionElement, declared: &impl Fn(&ast::ForInTarget) -> bool) -> bool {
    match c {
        ast::CollectionElement::ForIn { target, body, .. } => declared(target) || for_in_element(body, declared),
        ast::CollectionElement::For { body, .. } => for_in_element(body, declared),
        ast::CollectionElement::If { then, else_, .. } => for_in_element(then, declared) || else_.as_deref().is_some_and(|o| for_in_element(o, declared)),
        _ => false,
    }
}

/// A documentation comment (`///` or `/** */`) ends right before `start`
/// (only whitespace between): it belongs to the declaration's node.
fn doc_comment_before(source: &str, start: usize) -> bool {
    let before = source[..start].trim_end();
    if before.ends_with("*/") {
        return before.rfind("/*").is_some_and(|i| before[i..].starts_with("/**"));
    }
    let line = before.rsplit('\n').next().unwrap_or("").trim_start();
    line.starts_with("///")
}

/// The offset of the declaration of the innermost type parameter `name` in
/// scope at `offset` (in a declaration, function or function type that
/// contains it); `None` when none is in scope.
/// For each type parameter name of the unit, the spans of the declarations
/// that declare it: a name at `offset` is a type parameter when one of those
/// spans contains `offset` (the same criterion as
/// [`type_parameter_declaration`], without walking the tree by name).
pub(crate) fn type_parameter_scopes(ast: &Ast) -> std::collections::HashMap<SymbolId, Vec<Span>> {
    let mut m: std::collections::HashMap<SymbolId, Vec<Span>> = std::collections::HashMap::new();
    let mut by = |s: Span, ps: &[ast::TypeParameter]| {
        for t in ps {
            m.entry(t.name.sym).or_default().push(s);
        }
    };
    for d in &ast.decls {
        let ps = match &d.kind {
            DeclKind::Class(c) => &c.type_params,
            DeclKind::Mixin(m) => &m.type_params,
            DeclKind::Enum(e) => &e.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Typedef(t) => &t.type_params,
            _ => continue,
        };
        by(d.span, ps);
    }
    for f in &ast.functions {
        by(f.span, &f.type_params);
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind {
            by(t.span, type_params);
        }
    }
    m
}

pub(crate) fn type_parameter_declaration(
    ast: &Ast,
    offset: usize,
    name: SymbolId,
) -> Option<usize> {
    let find = |ps: &[ast::TypeParameter]| {
        ps.iter()
            .find(|t| t.name.sym == name)
            .map(|t| t.name.span.start)
    };
    let mut best: Option<(usize, usize)> = None;
    let mut consider = |s: Span, d: Option<usize>| {
        if let Some(d) = d
            && contains(s, offset)
            && best.is_none_or(|(size, _)| s.end - s.start < size)
        {
            best = Some((s.end - s.start, d));
        }
    };
    for d in &ast.decls {
        let ps = match &d.kind {
            DeclKind::Class(c) => &c.type_params,
            DeclKind::Mixin(m) => &m.type_params,
            DeclKind::Enum(e) => &e.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Typedef(t) => &t.type_params,
            _ => continue,
        };
        consider(d.span, find(ps));
    }
    for f in &ast.functions {
        consider(f.span, find(&f.type_params));
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind {
            consider(t.span, find(type_params));
        }
    }
    best.map(|(_, d)| d)
}

