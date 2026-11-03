//! The resolved-reference index (docs/LSP.md, "Décima quinta rodada"): what
//! every name of every known file resolves to, by an identity that survives
//! reloads, so that `textDocument/references` reads the answer instead of
//! loading and inferring every file that cites the name.
//!
//! **Identity.** A key is the declaration's library URI and its path in the
//! library: `dart:core#CString`, `dart:core#CString.gisNotEmpty`,
//! `package:a/b.dart#Fmain`. Two declarations of a library with the same key
//! (a duplicate declaration) make the key *ambiguous*, and a search that
//! touches it is answered the old way.
//!
//! **What a file records** ([`FileRecord`]), computed by [`Builder`] over a
//! program with the file's bodies inferred, with the very functions the search
//! uses ([`Project::unit_occurrences`], [`Project::resolve_doc_reference`],
//! [`Project::written_supertypes`]):
//! - the relations: each resolved name of an expression, type annotation,
//!   metadata, constructor written with the class name and `show`/`hide`, with
//!   the key it resolves to;
//! - the declarations (name spans) of the elements it declares, and the facts
//!   the member family needs: supertypes, written supertypes, members by base
//!   name, fields, the `this.x` names of its constructors;
//! - the `[ref]`s of its documentation, each prefix resolved;
//! - its `referencedNames` (the candidate files of `_addResults`);
//! - its line table, so locations come out without reading the file again.
//!
//! **The search** ([`ReferenceIndex::references`]) is the one of
//! `semantic::references_in`, over the records: the target is identified in
//! the document's library (the bounded session); the family of a member is
//! walked over the class facts; the uses are the relations of the candidate
//! files. It answers top-level elements and members. It gives up (and the
//! caller searches as before) for what it does not record: constructors,
//! prefixes, locals, members whose family has `this.x` parameters, an
//! ambiguous key, or a file whose record is not the current text. It never
//! answers partially.

use crate::project::{Concrete, Denoted, Owner, Project, Target, base_name, inside};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, LibraryId,
    TypedefId, UnitId, UnitRole, VariableId,
};
use dartforge_frontend::ast::{self, MemberKind};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Interned strings: the keys and the words (names) of the index.
#[derive(Debug, Default)]
pub(crate) struct Strings {
    ids: HashMap<Box<str>, u32>,
    texts: Vec<Box<str>>,
}

impl Strings {
    /// The id of `s`, interning it.
    pub(crate) fn id(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.ids.get(s) {
            return i;
        }
        let i = self.texts.len() as u32;
        self.texts.push(s.into());
        self.ids.insert(s.into(), i);
        i
    }

    /// The id of `s` when it is interned.
    pub(crate) fn get(&self, s: &str) -> Option<u32> {
        self.ids.get(s).copied()
    }

    fn text(&self, i: u32) -> &str {
        &self.texts[i as usize]
    }

    fn len(&self) -> usize {
        self.texts.len()
    }
}

/// What a relation records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Tag {
    /// A top-level element of an expression (`UnitOccurrences::elements`).
    Element,
    /// A type annotation or metadata name (`UnitOccurrences::type_names`).
    TypeName,
    /// A function or accessor (`UnitOccurrences::functions`).
    Function,
    /// A member variable (`UnitOccurrences::variables`).
    Variable,
    /// The class name of a constructor declaration.
    ConstructorClassName,
    /// A `show`/`hide` name of an import or export.
    Combinator,
    /// What cites a constructor ([`Project::unit_constructor_uses`]).
    Constructor,
    /// A `this.x` parameter or an `x = e` initializer of a class's
    /// constructor (key `<class>/p<x>`).
    FieldParameter,
    /// A named argument of a call to a constructor with that named `this.x`
    /// (key `<constructor>/l<x>`).
    Label,
    /// A positional argument to an optional `this.x` (key `<function>/a<i>`).
    Positional,
    /// A `super.x` parameter (key `<super constructor>/s<i>` for a positional
    /// one, `<super constructor>/n<x>` for a named one).
    Super,
}

/// A resolved name of a file: the key, the tag and the source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Relation {
    pub key: u32,
    pub tag: Tag,
    pub start: u32,
    pub end: u32,
}

/// What a documentation `[ref]` prefix resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DocTarget {
    /// A top-level element, with the elements its search covers
    /// ([`Project::top_elements`]).
    Top { key: u32, group: Vec<u32> },
    /// A member, by owner, base name and staticness.
    Member {
        owner: u32,
        name: u32,
        is_static: bool,
    },
    /// A constructor.
    Constructor { key: u32 },
    /// Something no top-level, member or constructor search matches by target
    /// (a local, a prefix); it can still match a member by its concrete
    /// element.
    Other,
}

/// The concrete element of a resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConcreteKey {
    Function(u32),
    Variable(u32),
}

/// A resolved prefix `r[..=i]` of a documentation reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocFact {
    /// The text of the prefix's last part (`r[i].1`).
    pub word: u32,
    pub start: u32,
    pub end: u32,
    /// The end of the previous part (`r[i - 1]`), [`NONE`] for the first: a
    /// constructor's `[A.name]` goes from there.
    pub previous_end: u32,
    pub target: DocTarget,
    pub concrete: Option<ConcreteKey>,
    /// [`Project::documented_field_parameter`] of the first part: the class of
    /// the field the `this.x` parameter names.
    pub field_parameter: Option<Option<u32>>,
}

/// The facts of a class or extension that the member family needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ClassFact {
    pub key: u32,
    /// The library's URI (a key string).
    pub library: u32,
    pub extension: bool,
    /// Superclass, mixins, interfaces and `on`, resolved.
    pub supers: Vec<u32>,
    /// [`Project::written_supertypes`].
    pub written: Vec<u32>,
    /// Instance and static members: base name, static, function key.
    pub members: Vec<(u32, bool, u32)>,
    /// Fields and enum constants: name, static, variable key.
    pub fields: Vec<(u32, bool, u32)>,
    /// The `this.x` parameters of its constructors.
    pub constructor_parameters: Vec<ConstructorParameter>,
}

/// A `this.x` parameter of a constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConstructorParameter {
    pub constructor: u32,
    /// The parameter's name (a word).
    pub name: u32,
    pub named: bool,
    /// A named one with another public name.
    pub public_name: bool,
    pub optional: bool,
    /// The position among the positional parameters.
    pub position: u32,
}

/// What the index keeps of one file.
#[derive(Debug, Clone, Default)]
pub(crate) struct FileRecord {
    /// The canonical path ([`dartforge_elements::generated::key`]).
    pub path: PathBuf,
    pub uri: String,
    /// The library's URI (a key string).
    pub library: u32,
    /// Outside the project root (SDK, packages): it does not change while the
    /// package configuration stays.
    pub frozen: bool,
    /// Hash and length of the text the record came from.
    pub stamp: u64,
    pub length: u32,
    /// The file's modification time (nanoseconds since the epoch, 0 when
    /// unknown) when the record was made.
    pub modified: u64,
    /// The text was the disk's (not an open document's other text): the
    /// same date and size mean the same text.
    pub disk: bool,
    /// [`dartforge_elements::incremental::api_signature_of`] of that text.
    pub api: Option<u64>,
    /// Line starts.
    pub lines: Vec<u32>,
    /// For each character wider in UTF-8 than in UTF-16: the offset after it
    /// and the bytes saved so far.
    pub wide: Vec<(u32, u32)>,
    pub referenced: Vec<u32>,
    /// Sorted by key, then start.
    pub relations: Vec<Relation>,
    /// Key and name span of the elements the file declares.
    pub declarations: Vec<(u32, u32, u32)>,
    /// On a library's first record: the keys of the elements its SDK patch
    /// files declare (they have no uses, but their library is a candidate).
    pub patch_declarations: Vec<u32>,
    pub docs: Vec<DocFact>,
    pub classes: Vec<ClassFact>,
    /// Function key → the key of its variable (implicit accessors).
    pub function_variables: Vec<(u32, u32)>,
    /// Variable key → its getter and setter keys (`u32::MAX`: none).
    pub accessors: Vec<(u32, u32, u32)>,
    /// Keys the file's library declares more than once.
    pub ambiguous: Vec<u32>,
    /// On a library's defining unit: the paths of the libraries it imports or
    /// exports (the dependents of an API change).
    pub imports: Vec<PathBuf>,
}

const NONE: u32 = u32::MAX;

impl FileRecord {
    /// The LSP position (line, UTF-16 column) of `offset`.
    pub(crate) fn position(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.length);
        let line = self
            .lines
            .partition_point(|s| *s <= offset)
            .saturating_sub(1);
        let start = self.lines.get(line).copied().unwrap_or(0);
        let saved = |at: u32| -> u32 {
            let i = self.wide.partition_point(|(o, _)| *o <= at);
            if i == 0 { 0 } else { self.wide[i - 1].1 }
        };
        (
            line as u32,
            (offset - start) - (saved(offset) - saved(start)),
        )
    }

    /// The relations with `key`.
    fn relations_of(&self, key: u32) -> &[Relation] {
        let from = self.relations.partition_point(|r| r.key < key);
        let to = self.relations.partition_point(|r| r.key <= key);
        &self.relations[from..to]
    }
}

/// The line starts and UTF-16 corrections of `text` (as
/// [`crate::utf16::LineTable`] counts them).
fn line_table(text: &str) -> (Vec<u32>, Vec<(u32, u32)>) {
    let b = text.as_bytes();
    let mut lines = vec![0u32];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\n' => {
                lines.push(i as u32 + 1);
                i += 1;
            }
            b'\r' => {
                if b.get(i + 1) == Some(&b'\n') {
                    lines.push(i as u32 + 2);
                    i += 2;
                } else {
                    lines.push(i as u32 + 1);
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    let mut wide = Vec::new();
    let mut saved = 0u32;
    if !text.is_ascii() {
        for (o, c) in text.char_indices() {
            let extra = (c.len_utf8() - c.len_utf16()) as u32;
            if extra > 0 {
                saved += extra;
                wide.push(((o + c.len_utf8()) as u32, saved));
            }
        }
    }
    (lines, wide)
}

/// The modification time (nanoseconds since the epoch, 0 when unknown) and
/// the size of a file.
pub(crate) fn file_date(path: &Path) -> (u64, Option<u64>) {
    let Ok(m) = std::fs::metadata(path) else {
        return (0, None);
    };
    let modified = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64);
    (modified, Some(m.len()))
}

/// A stable hash of a text (the same in every process).
pub(crate) fn text_stamp(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// The keys of the elements of one program.
struct Keys<'a> {
    project: &'a Project,
    unnamed_extensions: HashMap<LibraryId, Vec<ExtensionId>>,
}

impl<'a> Keys<'a> {
    fn new(project: &'a Project) -> Self {
        Keys {
            project,
            unnamed_extensions: HashMap::new(),
        }
    }

    fn uri(&self, lib: LibraryId) -> &str {
        &self.project.program().library(lib).uri
    }

    fn patch(&self, unit: Option<UnitId>) -> &'static str {
        match unit {
            Some(u) if self.project.program().unit(u).role == UnitRole::Patch => "!p",
            _ => "",
        }
    }

    fn class(&self, c: ClassId) -> String {
        let p = self.project.program();
        let cl = p.class(c);
        format!(
            "{}#C{}{}",
            self.uri(cl.library),
            self.project.name(cl.name),
            self.patch(cl.decl.map(|d| d.unit))
        )
    }

    fn extension(&mut self, x: ExtensionId) -> String {
        let p = self.project.program();
        let e = p.extension(x);
        let patch = self.patch(Some(e.decl.unit));
        match e.name {
            Some(n) => format!("{}#X{}{}", self.uri(e.library), self.project.name(n), patch),
            None => {
                let list = self.unnamed_extensions.entry(e.library).or_insert_with(|| {
                    (0..p.extensions.len())
                        .map(|i| ExtensionId(i as u32))
                        .filter(|y| {
                            p.extension(*y).library == e.library && p.extension(*y).name.is_none()
                        })
                        .collect()
                });
                let k = list.iter().position(|y| *y == x).unwrap_or(0);
                format!("{}#X#{}{}", self.uri(e.library), k, patch)
            }
        }
    }

    fn typedef(&self, t: TypedefId) -> String {
        let p = self.project.program();
        let d = p.typedef(t);
        format!(
            "{}#T{}{}",
            self.uri(d.library),
            self.project.name(d.name),
            self.patch(Some(d.decl.unit))
        )
    }

    fn owner(
        &mut self,
        class: Option<ClassId>,
        extension: Option<ExtensionId>,
        lib: LibraryId,
    ) -> String {
        match (class, extension) {
            (Some(c), _) => format!("{}.", self.class(c)),
            (_, Some(x)) => format!("{}.", self.extension(x)),
            _ => format!("{}#", self.uri(lib)),
        }
    }

    fn function(&mut self, f: FunctionElementId) -> String {
        let p = self.project.program();
        let fe = p.function(f);
        let kind = match fe.kind {
            FunctionKind::Function => 'f',
            FunctionKind::Getter => 'g',
            FunctionKind::Setter => 's',
            FunctionKind::Operator => 'o',
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => 'k',
            // The implicit setter has the getter's name.
            FunctionKind::ImplicitAccessor => {
                if fe.variable.is_some_and(|v| p.variable(v).setter == Some(f)) {
                    'j'
                } else {
                    'i'
                }
            }
        };
        let unit = match fe.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => {
                Some(unit)
            }
            FunctionRef::None => fe.variable.and_then(|v| p.variable(v).node.unit()),
        };
        let patch = self.patch(unit);
        let owner = self.owner(fe.class, fe.extension, fe.library);
        format!("{owner}{kind}{}{patch}", self.project.name(fe.name))
    }

    fn variable(&mut self, v: VariableId) -> String {
        let p = self.project.program();
        let ve = p.variable(v);
        let patch = self.patch(ve.node.unit());
        let owner = self.owner(ve.class, ve.extension, ve.library);
        format!("{owner}v{}{patch}", self.project.name(ve.name))
    }

    fn element(&mut self, el: Element) -> Option<String> {
        Some(match el {
            Element::Class(c) => self.class(c),
            Element::Extension(x) => self.extension(x),
            Element::Typedef(t) => self.typedef(t),
            Element::Function(f) => self.function(f),
            Element::Variable(v) => self.variable(v),
            Element::Prefix(..) => return None,
        })
    }
}

/// An element a unit declares.
#[derive(Debug, Clone, Copy)]
enum Declared {
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(TypedefId),
    Function(FunctionElementId),
    Variable(VariableId),
}

/// Builds the [`FileRecord`]s of the libraries of one program.
pub(crate) struct Builder;

/// The program and the keys of one [`Builder::library`] call.
struct Emit<'a> {
    project: &'a Project,
    keys: Keys<'a>,
}

/// The elements `lib` declares, by declaring unit (the program's
/// [`dartforge_elements::model::ElementIndex`] lists, in id order).
fn declared_by_unit(project: &Project, lib: LibraryId) -> HashMap<UnitId, Vec<Declared>> {
    let p = project.program();
    let index = p.index();
    let mut by_unit: HashMap<UnitId, Vec<Declared>> = HashMap::new();
    let first_unit = p.library(lib).units.first().copied();
    let mut put = |u: Option<UnitId>, d: Declared| {
        if let Some(u) = u {
            by_unit.entry(u).or_default().push(d);
        }
    };
    for &c in index
        .library_classes
        .get(lib.0 as usize)
        .into_iter()
        .flatten()
    {
        put(
            p.class(c).decl.map(|d| d.unit).or(first_unit),
            Declared::Class(c),
        );
    }
    for &u in &p.library(lib).units {
        let mut extensions: Vec<ExtensionId> = index
            .unit_extensions
            .get(u.0 as usize)
            .map(|m| m.values().copied().collect())
            .unwrap_or_default();
        extensions.sort();
        for x in extensions {
            put(Some(p.extension(x).decl.unit), Declared::Extension(x));
        }
        let mut typedefs: Vec<TypedefId> = index
            .unit_typedefs
            .get(u.0 as usize)
            .map(|m| m.values().copied().collect())
            .unwrap_or_default();
        typedefs.sort();
        for t in typedefs {
            put(Some(p.typedef(t).decl.unit), Declared::Typedef(t));
        }
    }
    for &g in index
        .library_functions
        .get(lib.0 as usize)
        .into_iter()
        .flatten()
    {
        let f = p.function(g);
        let unit = match f.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => {
                Some(unit)
            }
            FunctionRef::None => f
                .variable
                .and_then(|v| p.variable(v).node.unit())
                .or_else(|| f.class.and_then(|c| p.class(c).decl.map(|d| d.unit)))
                .or(first_unit),
        };
        put(unit, Declared::Function(g));
    }
    for &v in index
        .library_variables
        .get(lib.0 as usize)
        .into_iter()
        .flatten()
    {
        put(
            p.variable(v).node.unit().or(first_unit),
            Declared::Variable(v),
        );
    }
    by_unit
}

impl Builder {
    /// The records of the units of `lib` (not the patches), whose bodies must
    /// be inferred. Keys and words go to `keys` and `words`.
    pub(crate) fn library(
        &self,
        project: &Project,
        lib: LibraryId,
        root: &Path,
        keys: &mut Strings,
        words: &mut Strings,
    ) -> Vec<FileRecord> {
        let mut emit = Emit {
            project,
            keys: Keys::new(project),
        };
        let by_unit = declared_by_unit(project, lib);
        emit.library(&by_unit, lib, root, keys, words)
    }
}

impl Emit<'_> {
    fn library(
        &mut self,
        by_unit: &HashMap<UnitId, Vec<Declared>>,
        lib: LibraryId,
        root: &Path,
        keys: &mut Strings,
        words: &mut Strings,
    ) -> Vec<FileRecord> {
        let project = self.project;
        let p = project.program();
        let library = p.library(lib);
        let library_key = keys.id(&library.uri);
        let mut records: Vec<(UnitId, FileRecord)> = Vec::new();
        for &u in &library.units {
            let unit = p.unit(u);
            if unit.role == UnitRole::Patch {
                continue;
            }
            let Some(path) = unit.path.as_deref() else {
                continue;
            };
            let path = dartforge_elements::generated::key(path);
            let Some(uri) = url::Url::from_file_path(&path).ok().map(|u| u.to_string()) else {
                continue;
            };
            let (lines, wide) = line_table(&unit.source);
            let mut referenced: Vec<u32> = project
                .unit_referenced_names(u)
                .iter()
                .map(|n| words.id(n))
                .collect();
            referenced.sort_unstable();
            let frozen = !inside(&path, root);
            let (modified, disk_length) = file_date(&path);
            // A project file's record is of the disk only when the text is the
            // disk's (an open document may differ); the SDK and the packages are.
            let disk = disk_length == Some(unit.source.len() as u64)
                && (frozen
                    || std::fs::read_to_string(&path)
                        .is_ok_and(|t| text_stamp(&t) == text_stamp(&unit.source)));
            records.push((
                u,
                FileRecord {
                    frozen,
                    modified,
                    disk,
                    path,
                    uri,
                    library: library_key,
                    stamp: text_stamp(&unit.source),
                    length: unit.source.len() as u32,
                    api: dartforge_elements::incremental::api_signature_of(
                        &unit.source,
                        &unit.ast,
                        &unit.unit,
                    ),
                    lines,
                    wide,
                    referenced,
                    ..Default::default()
                },
            ));
        }
        let at =
            |records: &[(UnitId, FileRecord)], u: UnitId| records.iter().position(|(x, _)| *x == u);
        // The libraries it imports and exports, on the defining unit.
        if let Some(i) = library.units.first().and_then(|u| at(&records, *u)) {
            let mut imports: Vec<PathBuf> = library
                .imports
                .iter()
                .map(|i| i.library)
                .chain(library.exports.iter().map(|e| e.library))
                .filter_map(|l| {
                    let first = *p.library(l).units.first()?;
                    p.unit(first)
                        .path
                        .as_deref()
                        .map(dartforge_elements::generated::key)
                })
                .collect();
            imports.sort();
            imports.dedup();
            records[i].1.imports = imports;
        }
        // Declarations, class facts and accessors.
        let mut seen: HashMap<String, usize> = HashMap::new();
        for &u in &library.units {
            let declared = by_unit.get(&u).cloned().unwrap_or_default();
            for d in declared {
                let (key, name) = match d {
                    Declared::Class(c) => {
                        let key = self.keys.class(c);
                        let mut field_spans = Vec::new();
                        let fact = self.class_fact(c, keys, words, &mut field_spans);
                        for (fu, k, span) in field_spans {
                            if let Some(i) = at(&records, fu) {
                                records[i].1.relations.push(Relation {
                                    key: keys.id(&k),
                                    tag: Tag::FieldParameter,
                                    start: span.start as u32,
                                    end: span.end as u32,
                                });
                            }
                        }
                        // Constructors written with the class name.
                        let symbol = p.class(c).name;
                        for (cu, m) in p.class_members(c) {
                            if let MemberKind::Constructor(k) = &p.unit(cu).ast.member(m).kind
                                && k.class_name.sym == symbol
                                && let Some(i) = at(&records, cu)
                            {
                                records[i].1.relations.push(Relation {
                                    key: keys.id(&key),
                                    tag: Tag::ConstructorClassName,
                                    start: k.class_name.span.start as u32,
                                    end: k.class_name.span.end as u32,
                                });
                            }
                        }
                        if let Some(i) = at(&records, u) {
                            records[i].1.classes.push(fact);
                        }
                        (key, project.top_element_name(Element::Class(c)))
                    }
                    Declared::Extension(x) => {
                        let key = self.keys.extension(x);
                        let fact = self.extension_fact(x, keys, words);
                        if let Some(i) = at(&records, u) {
                            records[i].1.classes.push(fact);
                        }
                        (key, project.top_element_name(Element::Extension(x)))
                    }
                    Declared::Typedef(t) => (
                        self.keys.typedef(t),
                        project.top_element_name(Element::Typedef(t)),
                    ),
                    Declared::Function(f) => {
                        let key = self.keys.function(f);
                        if let Some(v) = p.function(f).variable
                            && let Some(i) = at(&records, u)
                        {
                            let vk = self.keys.variable(v);
                            records[i]
                                .1
                                .function_variables
                                .push((keys.id(&key), keys.id(&vk)));
                        }
                        (key, project.function_name(f))
                    }
                    Declared::Variable(v) => {
                        let key = self.keys.variable(v);
                        let ve = p.variable(v);
                        let getter = ve.getter.map(|g| self.keys.function(g));
                        let setter = ve.setter.map(|s| self.keys.function(s));
                        if let Some(i) = at(&records, u) {
                            let k = keys.id(&key);
                            let g = getter.map_or(NONE, |g| keys.id(&g));
                            let s = setter.map_or(NONE, |s| keys.id(&s));
                            records[i].1.accessors.push((k, g, s));
                        }
                        (key, project.variable_name(v))
                    }
                };
                *seen.entry(key.clone()).or_default() += 1;
                match name {
                    Some((du, span)) if at(&records, du).is_some() => {
                        let i = at(&records, du).expect("checked");
                        records[i].1.declarations.push((
                            keys.id(&key),
                            span.start as u32,
                            span.end as u32,
                        ));
                    }
                    Some((du, _)) if p.unit(du).role == UnitRole::Patch => {
                        if let Some(first) = records.first_mut() {
                            first.1.patch_declarations.push(keys.id(&key));
                        }
                    }
                    _ => {}
                }
            }
        }
        let ambiguous: Vec<u32> = seen
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(k, _)| keys.id(&k))
            .collect();
        if let Some(first) = records.first_mut() {
            first.1.ambiguous = ambiguous;
        }
        // Combinators of the library's imports and exports.
        let directives: Vec<(UnitId, usize, LibraryId)> = library
            .imports
            .iter()
            .map(|i| (i.unit, i.directive, i.library))
            .chain(
                library
                    .exports
                    .iter()
                    .map(|e| (e.unit, e.directive, e.library)),
            )
            .collect();
        for (u, d, target) in directives {
            let Some(i) = at(&records, u) else { continue };
            let Some(dir) = p.unit(u).unit.directives.get(d) else {
                continue;
            };
            let combinators = match &dir.kind {
                ast::DirectiveKind::Import { combinators, .. }
                | ast::DirectiveKind::Export { combinators, .. } => combinators,
                _ => continue,
            };
            for c in combinators {
                let (ast::Combinator::Show(names) | ast::Combinator::Hide(names)) = c;
                for n in names {
                    let Some(g) = p
                        .library(target)
                        .exported
                        .get(&n.sym)
                        .and_then(|b| b.getter.or(b.setter))
                    else {
                        continue;
                    };
                    if let Some(key) = self.keys.element(g) {
                        records[i].1.relations.push(Relation {
                            key: keys.id(&key),
                            tag: Tag::Combinator,
                            start: n.span.start as u32,
                            end: n.span.end as u32,
                        });
                    }
                }
            }
        }
        // Uses and documentation.
        for (u, record) in records.iter_mut() {
            self.uses(*u, record, keys);
            self.docs(*u, record, keys, words);
            record
                .relations
                .sort_by_key(|r| (r.key, r.start, r.end, r.tag));
            record.relations.dedup();
        }
        records.into_iter().map(|(_, r)| r).collect()
    }

    fn uses(&mut self, u: UnitId, record: &mut FileRecord, keys: &mut Strings) {
        let index = self.project.unit_occurrences(u);
        let mut push = |key: Option<String>, tag: Tag, spans: &[Span], keys: &mut Strings| {
            let Some(key) = key else { return };
            let k = keys.id(&key);
            for s in spans {
                record.relations.push(Relation {
                    key: k,
                    tag,
                    start: s.start as u32,
                    end: s.end as u32,
                });
            }
        };
        for (el, spans) in &index.elements {
            push(self.keys.element(*el), Tag::Element, spans, keys);
        }
        for (el, spans) in &index.type_names {
            push(self.keys.element(*el), Tag::TypeName, spans, keys);
        }
        for (f, spans) in &index.functions {
            push(Some(self.keys.function(*f)), Tag::Function, spans, keys);
        }
        for (v, spans) in &index.variables {
            push(Some(self.keys.variable(*v)), Tag::Variable, spans, keys);
        }
        for (g, s) in self.project.unit_constructor_uses(u) {
            push(Some(self.keys.function(g)), Tag::Constructor, &[s], keys);
        }
        let parameters = self.project.unit_parameter_uses(u);
        for &(g, label, s) in &parameters.labels {
            let k = format!("{}/l{}", self.keys.function(g), self.project.name(label));
            push(Some(k), Tag::Label, &[s], keys);
        }
        for &(g, i, s) in &parameters.positional {
            let k = format!("{}/a{i}", self.keys.function(g));
            push(Some(k), Tag::Positional, &[s], keys);
        }
        for &(sup, position, n, s) in &parameters.supers {
            let k = match position {
                Some(i) => format!("{}/s{i}", self.keys.function(sup)),
                None => format!("{}/n{}", self.keys.function(sup), self.project.name(n)),
            };
            push(Some(k), Tag::Super, &[s], keys);
        }
    }

    fn docs(
        &mut self,
        u: UnitId,
        record: &mut FileRecord,
        keys: &mut Strings,
        words: &mut Strings,
    ) {
        let project = self.project;
        let source = &project.program().unit(u).source;
        if !source.contains('[') {
            return;
        }
        for c in project.unit_comments(u).iter() {
            for r in &c.references {
                let only_constructor = r.len() <= 2
                    && matches!(
                        project.resolve_doc_reference(u, c.span, r),
                        Some((Target::Constructor(_), _))
                    );
                for i in 0..r.len() {
                    if only_constructor && i + 1 < r.len() {
                        continue;
                    }
                    let Some((a, concrete)) = project.resolve_doc_reference(u, c.span, &r[..=i])
                    else {
                        continue;
                    };
                    let target = match &a {
                        Target::Top(x) => match self.keys.element(*x) {
                            Some(key) => {
                                let group = project
                                    .top_elements(*x)
                                    .into_iter()
                                    .filter_map(|e| self.keys.element(e))
                                    .map(|k| keys.id(&k))
                                    .collect();
                                DocTarget::Top {
                                    key: keys.id(&key),
                                    group,
                                }
                            }
                            None => DocTarget::Other,
                        },
                        Target::Member {
                            owner,
                            name,
                            is_static,
                        } => {
                            let owner = match owner {
                                Owner::Class(c) => self.keys.class(*c),
                                Owner::Extension(x) => self.keys.extension(*x),
                            };
                            DocTarget::Member {
                                owner: keys.id(&owner),
                                name: words.id(name),
                                is_static: *is_static,
                            }
                        }
                        Target::Constructor(f) => DocTarget::Constructor {
                            key: keys.id(&self.keys.function(*f)),
                        },
                        _ => DocTarget::Other,
                    };
                    let concrete_key = match concrete {
                        Some(Concrete::Function(f)) => {
                            Some(ConcreteKey::Function(keys.id(&self.keys.function(f))))
                        }
                        Some(Concrete::Variable(v)) => {
                            Some(ConcreteKey::Variable(keys.id(&self.keys.variable(v))))
                        }
                        None => None,
                    };
                    if matches!(target, DocTarget::Other) && concrete_key.is_none() {
                        continue;
                    }
                    let field_parameter = if i == 0 {
                        project
                            .documented_field_parameter(u, c.span, &r[0].1, concrete)
                            .map(|class| class.map(|c| keys.id(&self.keys.class(c))))
                    } else {
                        None
                    };
                    record.docs.push(DocFact {
                        word: words.id(&r[i].1),
                        start: r[i].0.start as u32,
                        end: r[i].0.end as u32,
                        previous_end: if i > 0 { r[i - 1].0.end as u32 } else { NONE },
                        target,
                        concrete: concrete_key,
                        field_parameter,
                    });
                }
            }
        }
    }

    /// The facts of class `c`; the spans of its constructors' `this.x` and
    /// `x = e` go to `field_spans` (unit, key, span).
    fn class_fact(
        &mut self,
        c: ClassId,
        keys: &mut Strings,
        words: &mut Strings,
        field_spans: &mut Vec<(UnitId, String, Span)>,
    ) -> ClassFact {
        let project = self.project;
        let p = project.program();
        let cl = p.class(c);
        let supers: Vec<u32> = cl
            .supertype_class
            .iter()
            .chain(&cl.mixin_classes)
            .chain(&cl.interface_classes)
            .chain(&cl.on_classes)
            .map(|s| keys.id(&self.keys.class(*s)))
            .collect();
        let written: Vec<u32> = project
            .written_supertypes(c)
            .into_iter()
            .map(|s| keys.id(&self.keys.class(s)))
            .collect();
        let mut members = Vec::new();
        for (map, is_static) in [(&cl.instance_members, false), (&cl.static_members, true)] {
            for (s, f) in map {
                let name = words.id(base_name(project.name(*s)));
                members.push((name, is_static, keys.id(&self.keys.function(*f))));
            }
        }
        members.sort_unstable();
        let fields = cl
            .fields
            .iter()
            .chain(&cl.enum_constants)
            .map(|v| {
                let ve = p.variable(*v);
                (
                    words.id(project.name(ve.name)),
                    ve.static_,
                    keys.id(&self.keys.variable(*v)),
                )
            })
            .collect();
        let class_key = self.keys.class(c);
        let mut constructor_parameters = Vec::new();
        for (u, m) in p.class_members(c) {
            let MemberKind::Constructor(k) = &p.unit(u).ast.member(m).kind else {
                continue;
            };
            let constructor = project
                .node_constructor(u, m)
                .map(|f| keys.id(&self.keys.function(f)));
            let mut position = 0u32;
            for q in k.parameters.iter() {
                let named = q.kind == ast::ParameterKind::Named;
                if q.this_
                    && let Some(n) = q.name
                {
                    let name = project.name(n.sym);
                    field_spans.push((u, format!("{class_key}/p{name}"), n.span));
                    if let Some(constructor) = constructor {
                        constructor_parameters.push(ConstructorParameter {
                            constructor,
                            name: words.id(name),
                            named,
                            public_name: q.public_name.is_some(),
                            optional: q.kind == ast::ParameterKind::Optional,
                            position,
                        });
                    }
                }
                if !named {
                    position += 1;
                }
            }
            for i in k.initializers.iter() {
                if let ast::Initializer::Field { name, .. } = i {
                    let name_ = project.name(name.sym);
                    field_spans.push((u, format!("{class_key}/p{name_}"), name.span));
                }
            }
        }
        ClassFact {
            key: keys.id(&self.keys.class(c)),
            library: keys.id(&p.library(cl.library).uri),
            extension: false,
            supers,
            written,
            members,
            fields,
            constructor_parameters,
        }
    }

    fn extension_fact(
        &mut self,
        x: ExtensionId,
        keys: &mut Strings,
        words: &mut Strings,
    ) -> ClassFact {
        let project = self.project;
        let p = project.program();
        let e = p.extension(x);
        let mut members = Vec::new();
        for (map, is_static) in [(&e.instance_members, false), (&e.static_members, true)] {
            for (s, f) in map {
                let name = words.id(base_name(project.name(*s)));
                members.push((name, is_static, keys.id(&self.keys.function(*f))));
            }
        }
        members.sort_unstable();
        let fields = e
            .fields
            .iter()
            .map(|v| {
                let ve = p.variable(*v);
                (
                    words.id(project.name(ve.name)),
                    ve.static_,
                    keys.id(&self.keys.variable(*v)),
                )
            })
            .collect();
        ClassFact {
            key: keys.id(&self.keys.extension(x)),
            library: keys.id(&p.library(e.library).uri),
            extension: true,
            members,
            fields,
            ..Default::default()
        }
    }
}

/// What a search over the records derives from all of them, rebuilt after the
/// records change.
#[derive(Debug, Default)]
struct Derived {
    /// Files by referenced name.
    by_word: HashMap<u32, Vec<usize>>,
    /// Files by library.
    by_library: HashMap<u32, Vec<usize>>,
    /// The declaration of each key: file, start, end.
    declarations: HashMap<u32, (usize, u32, u32)>,
    /// The library of each key declared in an SDK patch file.
    patch_declarations: HashMap<u32, u32>,
    /// Class and extension facts: file and index.
    classes: HashMap<u32, (usize, usize)>,
    /// Written subtypes of each class.
    subtypes: HashMap<u32, Vec<u32>>,
    function_variables: HashMap<u32, u32>,
    accessors: HashMap<u32, (u32, u32)>,
    ambiguous: HashSet<u32>,
}

/// The member family over the records ([`Project::family`]).
#[derive(Debug, Default)]
struct KeyFamily {
    functions: HashSet<u32>,
    variables: HashSet<u32>,
    classes: Vec<u32>,
}

/// One located use or declaration of an answer: URI, byte span and the LSP
/// range.
pub(crate) type Located = (String, Span, Option<(u32, u32, u32, u32)>);

/// The index of one project: the records of the project's files, of the
/// packages' `lib/` files and of the SDK libraries (the known files of a
/// non-local search).
#[derive(Debug, Default)]
pub(crate) struct ReferenceIndex {
    pub(crate) root: PathBuf,
    pub(crate) keys: Strings,
    pub(crate) words: Strings,
    files: Vec<Option<FileRecord>>,
    by_path: HashMap<PathBuf, usize>,
    derived: Option<Derived>,
    /// Project files no load records (a part without its library: no search
    /// reads it either), with the stamp of the text that was not recorded.
    pub(crate) absent: HashMap<PathBuf, u64>,
}

/// Why the index does not answer a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Miss {
    /// The target is not of a kind the index records.
    Unsupported(&'static str),
    /// A key the search needs is unknown or ambiguous.
    Key(String),
    /// These files' records are not of their current text.
    Stale(Vec<PathBuf>),
}

impl ReferenceIndex {
    /// An empty index of the project at `root`.
    pub(crate) fn new(root: PathBuf) -> Self {
        ReferenceIndex {
            root,
            ..Default::default()
        }
    }

    /// Adds or replaces the records of their files.
    pub(crate) fn put(&mut self, records: Vec<FileRecord>) {
        for r in records {
            self.absent.remove(&r.path);
            match self.by_path.get(&r.path) {
                Some(&i) => self.files[i] = Some(r),
                None => {
                    self.by_path.insert(r.path.clone(), self.files.len());
                    self.files.push(Some(r));
                }
            }
        }
        self.derived = None;
    }

    /// Drops the record of `path` (a file that no longer exists).
    pub(crate) fn remove(&mut self, path: &Path) {
        if let Some(&i) = self.by_path.get(path) {
            self.files[i] = None;
            self.by_path.remove(path);
            self.derived = None;
        }
    }

    /// The record of `path`.
    pub(crate) fn file(&self, path: &Path) -> Option<&FileRecord> {
        self.by_path.get(path).and_then(|&i| self.files[i].as_ref())
    }

    /// The records, with their slots.
    pub(crate) fn records(&self) -> impl Iterator<Item = &FileRecord> {
        self.files.iter().flatten()
    }

    /// Number of files, relations and keys (for the log).
    pub(crate) fn sizes(&self) -> (usize, usize, usize) {
        let files = self.records().count();
        let relations = self.records().map(|r| r.relations.len()).sum();
        (files, relations, self.keys.len())
    }

    fn ensure_derived(&mut self) {
        if self.derived.is_none() {
            let mut d = Derived::default();
            for (i, r) in self.files.iter().enumerate() {
                let Some(r) = r else { continue };
                for w in &r.referenced {
                    d.by_word.entry(*w).or_default().push(i);
                }
                d.by_library.entry(r.library).or_default().push(i);
                for &(k, s, e) in &r.declarations {
                    d.declarations.entry(k).or_insert((i, s, e));
                }
                for &k in &r.patch_declarations {
                    d.patch_declarations.entry(k).or_insert(r.library);
                }
                for (j, c) in r.classes.iter().enumerate() {
                    d.classes.entry(c.key).or_insert((i, j));
                    for &s in &c.written {
                        let v = d.subtypes.entry(s).or_default();
                        if !v.contains(&c.key) {
                            v.push(c.key);
                        }
                    }
                }
                for &(f, v) in &r.function_variables {
                    d.function_variables.insert(f, v);
                }
                for &(v, g, s) in &r.accessors {
                    d.accessors.insert(v, (g, s));
                }
                d.ambiguous.extend(r.ambiguous.iter().copied());
            }
            self.derived = Some(d);
        }
    }

    fn class_fact(&self, d: &Derived, key: u32) -> Option<&ClassFact> {
        let &(f, j) = d.classes.get(&key)?;
        self.files[f].as_ref()?.classes.get(j)
    }

    /// `declared(x, name, is_static)` over the facts.
    fn declared(&self, d: &Derived, x: u32, name: u32, is_static: bool) -> Vec<u32> {
        self.class_fact(d, x)
            .map(|c| {
                c.members
                    .iter()
                    .filter(|(n, s, _)| *n == name && *s == is_static)
                    .map(|(_, _, f)| *f)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// [`Project::family`] over the facts. `None` when a class it needs has no
    /// facts.
    fn family(&self, d: &Derived, owner: u32, name: &str, is_static: bool) -> Option<KeyFamily> {
        let mut family = KeyFamily::default();
        let fact = self.class_fact(d, owner)?;
        let word = self.words.get(name);
        if fact.extension || is_static {
            if let Some(w) = word {
                family
                    .functions
                    .extend(self.declared(d, owner, w, is_static));
                for &(n, s, v) in &fact.fields {
                    if n == w && (fact.extension || s) {
                        family.variables.insert(v);
                    }
                }
            }
            if !fact.extension {
                family.classes.push(owner);
            }
        } else {
            let library = fact.library;
            let private = name.starts_with('_');
            let declares = |x: u32| {
                word.is_some_and(|w| {
                    !self.declared(d, x, w, false).is_empty()
                        || !self.declared(d, x, w, true).is_empty()
                })
            };
            let same_library = |x: u32| self.class_fact(d, x).is_some_and(|c| c.library == library);
            // The supertypes, transitively.
            let mut supers: Vec<u32> = Vec::new();
            let mut stack = vec![owner];
            while let Some(x) = stack.pop() {
                for &s in self.class_fact(d, x).map(|c| &c.supers[..]).unwrap_or(&[]) {
                    if !supers.contains(&s) {
                        supers.push(s);
                        stack.push(s);
                    }
                }
            }
            supers.retain(|&s| !private || same_library(s));
            let mut descent: Vec<u32> = Vec::new();
            for x in supers.into_iter().chain([owner]) {
                if !declares(x) {
                    continue;
                }
                let mut stack = vec![x];
                while let Some(y) = stack.pop() {
                    for &s in d.subtypes.get(&y).map(|v| &v[..]).unwrap_or(&[]) {
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
                descent.retain(|&s| same_library(s));
            }
            if let Some(w) = word {
                for &x in &descent {
                    for f in self
                        .declared(d, x, w, false)
                        .into_iter()
                        .chain(self.declared(d, x, w, true))
                    {
                        family.functions.insert(f);
                        if let Some(v) = d.function_variables.get(&f) {
                            family.variables.insert(*v);
                        }
                    }
                }
            }
            family.classes = descent;
        }
        for v in family.variables.clone() {
            if let Some(&(g, s)) = d.accessors.get(&v) {
                family
                    .functions
                    .extend([g, s].into_iter().filter(|k| *k != NONE));
            }
        }
        Some(family)
    }

    /// The relations a field's search adds (`member_occurrences` without the
    /// whole family's parameters): the `this.x` and `x = e` of the family's
    /// classes, the labels of the calls to the constructors with a named
    /// `this.x`, and, when the owner declares the field, the positional
    /// arguments to its optional `this.x`s and the `super.x`s that point to
    /// its `this.x`s.
    fn field_parameter_relations(
        &self,
        d: &Derived,
        family: &KeyFamily,
        owner: u32,
        name: &str,
    ) -> Vec<(u32, Tag)> {
        let mut output = Vec::new();
        let Some(word) = self.words.get(name) else {
            return output;
        };
        let key = |text: String, tag: Tag, output: &mut Vec<(u32, Tag)>| {
            if let Some(k) = self.keys.get(&text) {
                output.push((k, tag));
            }
        };
        let mut named = Vec::new();
        for &c in &family.classes {
            key(
                format!("{}/p{name}", self.keys.text(c)),
                Tag::FieldParameter,
                &mut output,
            );
            for q in self
                .class_fact(d, c)
                .map(|f| &f.constructor_parameters[..])
                .unwrap_or(&[])
            {
                if q.name == word && q.named && !q.public_name && !named.contains(&q.constructor) {
                    named.push(q.constructor);
                }
            }
        }
        for g in named {
            key(
                format!("{}/l{name}", self.keys.text(g)),
                Tag::Label,
                &mut output,
            );
        }
        let Some(fact) = self.class_fact(d, owner) else {
            return output;
        };
        if fact.extension || !fact.fields.iter().any(|&(n, s, _)| n == word && !s) {
            return output;
        }
        for q in &fact.constructor_parameters {
            if q.name != word {
                continue;
            }
            let g = self.keys.text(q.constructor);
            if q.named {
                key(format!("{g}/n{name}"), Tag::Super, &mut output);
            } else {
                key(format!("{g}/s{}", q.position), Tag::Super, &mut output);
                if q.optional {
                    key(format!("{g}/a{}", q.position), Tag::Positional, &mut output);
                }
            }
        }
        output
    }

    /// The references of what `denoted` is in `session` (the document's
    /// library, with its bodies inferred), as `semantic::references_in`
    /// gives them over the wide load. `Err` when the index does not answer.
    ///
    /// `fresh(record)` says whether a project file's record is of its
    /// current text.
    pub(crate) fn references(
        &mut self,
        session: &Project,
        denoted: &Denoted,
        fresh: &dyn Fn(&FileRecord) -> bool,
    ) -> Result<(Option<Located>, Vec<Located>), Miss> {
        self.ensure_derived();
        let this: &Self = self;
        this.search(session, denoted, fresh)
    }

    fn search(
        &self,
        session: &Project,
        denoted: &Denoted,
        fresh: &dyn Fn(&FileRecord) -> bool,
    ) -> Result<(Option<Located>, Vec<Located>), Miss> {
        let d = self.derived.as_ref().expect("ensured by the caller");
        let target = &denoted.target;
        let name = session
            .search_name(target)
            .ok_or(Miss::Unsupported("a local search"))?;
        let word = session.target_word(target);
        let mut keys = Keys::new(session);
        // What the target covers, by key.
        enum Covers {
            Top(HashSet<u32>),
            Member {
                family: KeyFamily,
                owner: u32,
                is_static: bool,
                /// The relations of the field's `this.x` search (`member_occurrences`).
                parameters: Vec<(u32, Tag)>,
            },
            Constructor(u32),
        }
        let key_of = |index: &Self, k: String| -> Result<u32, Miss> {
            index.keys.get(&k).ok_or(Miss::Key(k))
        };
        let covers = match target {
            Target::Top(el) => {
                let mut set = HashSet::new();
                for e in session.top_elements(*el) {
                    let k = keys.element(e).ok_or(Miss::Unsupported("a prefix"))?;
                    set.insert(key_of(self, k)?);
                }
                Covers::Top(set)
            }
            Target::Member {
                owner,
                name: member,
                is_static,
            } => {
                let owner = match owner {
                    Owner::Class(c) => keys.class(*c),
                    Owner::Extension(x) => keys.extension(*x),
                };
                let owner = key_of(self, owner)?;
                let family = self
                    .family(d, owner, member, *is_static)
                    .ok_or(Miss::Unsupported("a class without facts"))?;
                let parameters = if *is_static {
                    Vec::new()
                } else {
                    self.field_parameter_relations(d, &family, owner, member)
                };
                Covers::Member {
                    family,
                    owner,
                    is_static: *is_static,
                    parameters,
                }
            }
            Target::Constructor(f) => Covers::Constructor(key_of(self, keys.function(*f))?),
            _ => return Err(Miss::Unsupported("a local search")),
        };
        // Ambiguous keys.
        let involved: Vec<u32> = match &covers {
            Covers::Top(set) => set.iter().copied().collect(),
            Covers::Constructor(k) => vec![*k],
            Covers::Member { family, owner, .. } => family
                .functions
                .iter()
                .chain(&family.variables)
                .chain(&family.classes)
                .chain([owner])
                .copied()
                .collect(),
        };
        if let Some(k) = involved.iter().find(|k| d.ambiguous.contains(k)) {
            return Err(Miss::Key(self.keys.text(*k).to_string()));
        }
        // The declarations, which are not uses.
        let mut declarations: HashSet<(usize, u32, u32)> = HashSet::new();
        let located = |index: &Self, f: usize, s: u32, e: u32| -> Option<Located> {
            let r = index.files[f].as_ref()?;
            let (l0, c0) = r.position(s);
            let (l1, c1) = r.position(e);
            Some((
                r.uri.clone(),
                Span {
                    start: s as usize,
                    end: e as usize,
                },
                Some((l0, c0, l1, c1)),
            ))
        };
        // The record of a unit of the session, by its path as loaded.
        let file_of = |index: &Self, u: UnitId| -> Option<usize> {
            let path = session.program().unit(u).path.as_deref()?;
            index
                .by_path
                .get(dartforge_elements::generated::key_of(path).as_ref())
                .copied()
        };
        // The declaring units, by file (`None`: an SDK patch, which has no
        // uses) and library.
        let mut declaration_files: Vec<(Option<usize>, u32)> = Vec::new();
        let declared_in = |u: UnitId| -> Result<(Option<usize>, u32), Miss> {
            let p = session.program();
            let uri = &p.library(p.unit(u).library).uri;
            let library = self
                .keys
                .get(uri)
                .ok_or_else(|| Miss::Key(format!("{uri}: a library outside the index")))?;
            if p.unit(u).role == UnitRole::Patch {
                return Ok((None, library));
            }
            let f = file_of(self, u)
                .ok_or_else(|| Miss::Key(format!("{uri}: a file outside the index")))?;
            Ok((Some(f), library))
        };
        match &covers {
            Covers::Top(_) | Covers::Constructor(_) => {
                for (u, s, e) in session.declarations(target) {
                    let (f, library) = declared_in(u)?;
                    if let Some(f) = f {
                        declarations.insert((f, s as u32, e as u32));
                    }
                    declaration_files.push((f, library));
                }
            }
            Covers::Member { family, .. } => {
                for k in family.functions.iter().chain(&family.variables) {
                    if let Some(&(f, s, e)) = d.declarations.get(k) {
                        declarations.insert((f, s, e));
                        if let Some(r) = self.files[f].as_ref() {
                            declaration_files.push((Some(f), r.library));
                        }
                    } else if let Some(&library) = d.patch_declarations.get(k) {
                        declaration_files.push((None, library));
                    }
                }
            }
        }
        if let Some((u, _)) = session.declaration(denoted) {
            declaration_files.push(declared_in(u)?);
        }
        // The candidate files of `_addResults`.
        let private = name.starts_with('_');
        let name_word = self.words.get(&name);
        let cites = |f: usize| {
            name_word.is_some_and(|w| {
                self.files[f]
                    .as_ref()
                    .is_some_and(|r| r.referenced.binary_search(&w).is_ok())
            })
        };
        let mut candidates: HashSet<usize> = HashSet::new();
        for &(f, lib) in &declaration_files {
            for &g in d.by_library.get(&lib).map(|v| &v[..]).unwrap_or(&[]) {
                if !private || Some(g) == f || cites(g) {
                    candidates.insert(g);
                }
            }
        }
        if !private && let Some(w) = name_word {
            candidates.extend(d.by_word.get(&w).into_iter().flatten().copied());
        }
        // Every candidate of the project must be of its current text.
        let stale: Vec<PathBuf> = candidates
            .iter()
            .filter_map(|&f| self.files[f].as_ref())
            .filter(|r| !r.frozen && !fresh(r))
            .map(|r| r.path.clone())
            .collect();
        if !stale.is_empty() {
            return Err(Miss::Stale(stale));
        }
        let word_id = word.as_deref().map(|w| self.words.get(w));
        let target_key = match target {
            Target::Top(el) => keys.element(*el).and_then(|k| self.keys.get(&k)),
            _ => None,
        };
        // A documentation member target matches the searched member
        // (`same_target`: same name and staticness, and the same owner or
        // intersecting families).
        let mut families: HashMap<u32, Option<KeyFamily>> = HashMap::new();
        let mut uses: Vec<(usize, u32, u32)> = Vec::new();
        let mut candidates: Vec<usize> = candidates.into_iter().collect();
        candidates.sort_unstable();
        for &f in &candidates {
            let Some(r) = self.files[f].as_ref() else {
                continue;
            };
            match &covers {
                Covers::Top(set) => {
                    for k in set {
                        for rel in r.relations_of(*k) {
                            if matches!(
                                rel.tag,
                                Tag::Element
                                    | Tag::TypeName
                                    | Tag::ConstructorClassName
                                    | Tag::Combinator
                            ) {
                                uses.push((f, rel.start, rel.end));
                            }
                        }
                    }
                }
                Covers::Constructor(k) => {
                    for rel in r.relations_of(*k) {
                        if rel.tag == Tag::Constructor {
                            uses.push((f, rel.start, rel.end));
                        }
                    }
                }
                Covers::Member {
                    family, parameters, ..
                } => {
                    for &(k, tag) in parameters {
                        for rel in r.relations_of(k) {
                            if rel.tag == tag {
                                uses.push((f, rel.start, rel.end));
                            }
                        }
                    }
                    for k in &family.functions {
                        for rel in r.relations_of(*k) {
                            if rel.tag == Tag::Function {
                                uses.push((f, rel.start, rel.end));
                            }
                        }
                    }
                    for k in &family.variables {
                        for rel in r.relations_of(*k) {
                            if rel.tag == Tag::Variable {
                                uses.push((f, rel.start, rel.end));
                            }
                        }
                    }
                }
            }
            for doc in &r.docs {
                // Only the prefixes whose last part writes the target's name.
                if let Some(w) = &word_id
                    && *w != Some(doc.word)
                {
                    continue;
                }
                let matches = match (&covers, &doc.concrete) {
                    (Covers::Constructor(k), _) => {
                        matches!(doc.target, DocTarget::Constructor { key } if key == *k)
                    }
                    (Covers::Member { family, .. }, Some(ConcreteKey::Function(x))) => {
                        family.functions.contains(x)
                    }
                    (Covers::Member { family, .. }, Some(ConcreteKey::Variable(x))) => {
                        family.variables.contains(x)
                    }
                    // `same_target`: `x == y || top_elements(x).contains(y)`.
                    (Covers::Top(_), _) => match &doc.target {
                        DocTarget::Top { key, group } => {
                            target_key.is_some_and(|y| *key == y || group.contains(&y))
                        }
                        _ => false,
                    },
                    (
                        Covers::Member {
                            family,
                            owner,
                            is_static,
                            ..
                        },
                        None,
                    ) => match &doc.target {
                        DocTarget::Member {
                            owner: o,
                            name: n,
                            is_static: s,
                        } => {
                            self.words.text(*n) == name
                                && s == is_static
                                && (o == owner || {
                                    let other = families
                                        .entry(*o)
                                        .or_insert_with(|| self.family(d, *o, &name, *s));
                                    other.as_ref().is_some_and(|g| {
                                        !g.functions.is_disjoint(&family.functions)
                                            || !g.variables.is_disjoint(&family.variables)
                                    })
                                })
                        }
                        _ => false,
                    },
                };
                if !matches {
                    continue;
                }
                if let Some(class) = doc.field_parameter {
                    let own = match (&covers, target) {
                        (
                            Covers::Member { owner, .. },
                            Target::Member {
                                owner: Owner::Class(_),
                                ..
                            },
                        ) => class == Some(*owner),
                        _ => false,
                    };
                    if !own {
                        continue;
                    }
                }
                // A constructor's `[A.name]` goes from the end of `A`.
                let start = match covers {
                    Covers::Constructor(_) if doc.previous_end != NONE => doc.previous_end,
                    _ => doc.start,
                };
                uses.push((f, start, doc.end));
            }
        }
        uses.sort_unstable();
        uses.dedup();
        uses.retain(|u| !declarations.contains(u));
        let mut located_uses: Vec<Located> = uses
            .into_iter()
            .filter_map(|(f, s, e)| located(self, f, s, e))
            .collect();
        located_uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        let declaration = session
            .declaration_to_references(denoted)
            .and_then(|(u, span)| {
                let uri = session.unit_uri(u)?;
                match file_of(self, u) {
                    Some(f) => located(self, f, span.start as u32, span.end as u32),
                    None => Some((uri, span, None)),
                }
            });
        Ok((declaration, located_uses))
    }
}

/// Builds the index of the project of `uri`: the wide load of the non-local
/// search (the project, every SDK library and every `lib/` file of the
/// packages), each library's bodies inferred, recorded and dropped in turn,
/// so that only one library's tables are alive at a time.
pub(crate) fn build(
    sdk: &dartforge_elements::sdk::SdkLayout,
    documents: &crate::DocumentStore,
    uri: &str,
) -> Option<ReferenceIndex> {
    let file = crate::project::uri_file(uri)?;
    let root = crate::project::project_root(&file);
    let started = std::time::Instant::now();
    let mut project = crate::project::load_wide_project_with(sdk, documents, uri, None, false)?;
    let loaded = started.elapsed();
    let builder = Builder;
    let mut index = ReferenceIndex::new(root.clone());
    let libraries = project.program().libraries.len();
    for i in 0..libraries {
        let lib = LibraryId(i as u32);
        let units = project.program().library(lib).units.clone();
        let p = project.program();
        let known = units.iter().any(|u| {
            p.unit(*u)
                .path
                .as_deref()
                .is_some_and(|c| c.ends_with(crate::project::KNOWN))
        });
        let indexed = units
            .iter()
            .any(|u| p.unit(*u).role != UnitRole::Patch && p.unit(*u).path.is_some());
        if known || !indexed {
            continue;
        }
        project.query.infer_libraries(&[lib]);
        let records = builder.library(&project, lib, &root, &mut index.keys, &mut index.words);
        for u in units {
            project.query.bodies.units[u.0 as usize] = Default::default();
            project.forget_unit(u);
        }
        index.put(records);
    }
    index.note_absent(crate::project::project_files(&root).into_iter());
    if std::env::var_os("DARTFORGE_LSP_DEBUG").is_some() {
        let (files, relations, keys) = index.sizes();
        eprintln!(
            "[dartforge-lsp] reference index of {}: {files} files, {relations} relations, {keys} keys; load {} ms, total {} ms",
            root.display(),
            loaded.as_millis(),
            started.elapsed().as_millis()
        );
    }
    Some(index)
}

/// The version of [`ReferenceIndex::encode`]'s format (in the header).
const FORMAT: u32 = 1;

/// A little-endian LEB128 writer.
#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(b);
                return;
            }
            self.0.push(b | 0x80);
        }
    }

    fn n(&mut self, v: usize) {
        self.u(v as u64);
    }

    fn s(&mut self, t: &str) {
        self.n(t.len());
        self.0.extend_from_slice(t.as_bytes());
    }

    fn path(&mut self, p: &Path) {
        self.s(&p.to_string_lossy());
    }
}

/// The reader of [`Writer`]'s format; `None` at the end or on bad data.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u(&mut self) -> Option<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = *self.bytes.get(self.at)?;
            self.at += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
            shift += 7;
            if shift > 63 {
                return None;
            }
        }
    }

    fn w(&mut self) -> Option<u32> {
        u32::try_from(self.u()?).ok()
    }

    fn n(&mut self) -> Option<usize> {
        usize::try_from(self.u()?).ok()
    }

    fn s(&mut self) -> Option<&str> {
        let n = self.n()?;
        let b = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        std::str::from_utf8(b).ok()
    }

    fn path(&mut self) -> Option<PathBuf> {
        self.s().map(PathBuf::from)
    }

    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let n = self.n()?;
        let mut v = Vec::with_capacity(n.min(1 << 20));
        for _ in 0..n {
            v.push(item(self)?);
        }
        Some(v)
    }
}

fn tag_code(t: Tag) -> u64 {
    match t {
        Tag::Element => 0,
        Tag::TypeName => 1,
        Tag::Function => 2,
        Tag::Variable => 3,
        Tag::ConstructorClassName => 4,
        Tag::Combinator => 5,
        Tag::Constructor => 6,
        Tag::FieldParameter => 7,
        Tag::Label => 8,
        Tag::Positional => 9,
        Tag::Super => 10,
    }
}

fn tag_of(c: u64) -> Option<Tag> {
    Some(match c {
        0 => Tag::Element,
        1 => Tag::TypeName,
        2 => Tag::Function,
        3 => Tag::Variable,
        4 => Tag::ConstructorClassName,
        5 => Tag::Combinator,
        6 => Tag::Constructor,
        7 => Tag::FieldParameter,
        8 => Tag::Label,
        9 => Tag::Positional,
        10 => Tag::Super,
        _ => return None,
    })
}

/// `NONE` and the other ids, written one above so that `NONE` is 0.
fn opt(v: u32) -> u64 {
    if v == NONE { 0 } else { u64::from(v) + 1 }
}

fn unopt(v: u64) -> Option<u32> {
    if v == 0 {
        Some(NONE)
    } else {
        u32::try_from(v - 1).ok()
    }
}

fn write_record(w: &mut Writer, r: &FileRecord) {
    w.path(&r.path);
    w.s(&r.uri);
    w.u(u64::from(r.library));
    w.u(u64::from(r.frozen) | u64::from(r.disk) << 1);
    w.u(r.stamp);
    w.u(u64::from(r.length));
    w.u(r.modified);
    w.u(u64::from(r.api.is_some()));
    if let Some(a) = r.api {
        w.u(a);
    }
    let mut last = 0;
    w.n(r.lines.len());
    for &l in &r.lines {
        w.u(u64::from(l - last));
        last = l;
    }
    w.n(r.wide.len());
    for &(o, s) in &r.wide {
        w.u(u64::from(o));
        w.u(u64::from(s));
    }
    w.n(r.referenced.len());
    for &x in &r.referenced {
        w.u(u64::from(x));
    }
    w.n(r.relations.len());
    for x in &r.relations {
        w.u(u64::from(x.key));
        w.u(tag_code(x.tag));
        w.u(u64::from(x.start));
        w.u(u64::from(x.end - x.start));
    }
    w.n(r.declarations.len());
    for &(k, s, e) in &r.declarations {
        w.u(u64::from(k));
        w.u(u64::from(s));
        w.u(u64::from(e - s));
    }
    w.n(r.patch_declarations.len());
    for &k in &r.patch_declarations {
        w.u(u64::from(k));
    }
    w.n(r.docs.len());
    for d in &r.docs {
        w.u(u64::from(d.word));
        w.u(u64::from(d.start));
        w.u(u64::from(d.end - d.start));
        w.u(opt(d.previous_end));
        match &d.target {
            DocTarget::Top { key, group } => {
                w.u(0);
                w.u(u64::from(*key));
                w.n(group.len());
                for &g in group {
                    w.u(u64::from(g));
                }
            }
            DocTarget::Member {
                owner,
                name,
                is_static,
            } => {
                w.u(1);
                w.u(u64::from(*owner));
                w.u(u64::from(*name));
                w.u(u64::from(*is_static));
            }
            DocTarget::Constructor { key } => {
                w.u(2);
                w.u(u64::from(*key));
            }
            DocTarget::Other => w.u(3),
        }
        match d.concrete {
            None => w.u(0),
            Some(ConcreteKey::Function(k)) => {
                w.u(1);
                w.u(u64::from(k));
            }
            Some(ConcreteKey::Variable(k)) => {
                w.u(2);
                w.u(u64::from(k));
            }
        }
        match d.field_parameter {
            None => w.u(0),
            Some(c) => {
                w.u(1);
                w.u(c.map_or(0, |c| u64::from(c) + 1));
            }
        }
    }
    w.n(r.classes.len());
    for c in &r.classes {
        w.u(u64::from(c.key));
        w.u(u64::from(c.library));
        w.u(u64::from(c.extension));
        for list in [&c.supers, &c.written] {
            w.n(list.len());
            for &k in list {
                w.u(u64::from(k));
            }
        }
        for list in [&c.members, &c.fields] {
            w.n(list.len());
            for &(n, s, k) in list {
                w.u(u64::from(n));
                w.u(u64::from(s));
                w.u(u64::from(k));
            }
        }
        w.n(c.constructor_parameters.len());
        for q in &c.constructor_parameters {
            w.u(u64::from(q.constructor));
            w.u(u64::from(q.name));
            w.u(u64::from(q.named) | u64::from(q.public_name) << 1 | u64::from(q.optional) << 2);
            w.u(u64::from(q.position));
        }
    }
    w.n(r.function_variables.len());
    for &(f, v) in &r.function_variables {
        w.u(u64::from(f));
        w.u(u64::from(v));
    }
    w.n(r.accessors.len());
    for &(v, g, s) in &r.accessors {
        w.u(u64::from(v));
        w.u(opt(g));
        w.u(opt(s));
    }
    w.n(r.ambiguous.len());
    for &k in &r.ambiguous {
        w.u(u64::from(k));
    }
    w.n(r.imports.len());
    for i in &r.imports {
        w.path(i);
    }
}

fn read_record(r: &mut Reader<'_>) -> Option<FileRecord> {
    let path = r.path()?;
    let uri = r.s()?.to_string();
    let library = r.w()?;
    let flags = r.u()?;
    let stamp = r.u()?;
    let length = r.w()?;
    let modified = r.u()?;
    let api = if r.u()? != 0 { Some(r.u()?) } else { None };
    let mut last = 0u32;
    let lines = r.list(|r| {
        last = last.checked_add(r.w()?)?;
        Some(last)
    })?;
    let wide = r.list(|r| Some((r.w()?, r.w()?)))?;
    let referenced = r.list(|r| r.w())?;
    let relations = r.list(|r| {
        let key = r.w()?;
        let tag = tag_of(r.u()?)?;
        let start = r.w()?;
        let end = start.checked_add(r.w()?)?;
        Some(Relation {
            key,
            tag,
            start,
            end,
        })
    })?;
    let declarations = r.list(|r| {
        let k = r.w()?;
        let s = r.w()?;
        Some((k, s, s.checked_add(r.w()?)?))
    })?;
    let patch_declarations = r.list(|r| r.w())?;
    let docs = r.list(|r| {
        let word = r.w()?;
        let start = r.w()?;
        let end = start.checked_add(r.w()?)?;
        let previous_end = unopt(r.u()?)?;
        let target = match r.u()? {
            0 => {
                let key = r.w()?;
                let group = r.list(|r| r.w())?;
                DocTarget::Top { key, group }
            }
            1 => DocTarget::Member {
                owner: r.w()?,
                name: r.w()?,
                is_static: r.u()? != 0,
            },
            2 => DocTarget::Constructor { key: r.w()? },
            3 => DocTarget::Other,
            _ => return None,
        };
        let concrete = match r.u()? {
            0 => None,
            1 => Some(ConcreteKey::Function(r.w()?)),
            2 => Some(ConcreteKey::Variable(r.w()?)),
            _ => return None,
        };
        let field_parameter = match r.u()? {
            0 => None,
            1 => Some(match r.u()? {
                0 => None,
                c => Some(u32::try_from(c - 1).ok()?),
            }),
            _ => return None,
        };
        Some(DocFact {
            word,
            start,
            end,
            previous_end,
            target,
            concrete,
            field_parameter,
        })
    })?;
    let classes = r.list(|r| {
        let key = r.w()?;
        let library = r.w()?;
        let extension = r.u()? != 0;
        let supers = r.list(|r| r.w())?;
        let written = r.list(|r| r.w())?;
        let members = r.list(|r| Some((r.w()?, r.u()? != 0, r.w()?)))?;
        let fields = r.list(|r| Some((r.w()?, r.u()? != 0, r.w()?)))?;
        let constructor_parameters = r.list(|r| {
            let constructor = r.w()?;
            let name = r.w()?;
            let flags = r.u()?;
            Some(ConstructorParameter {
                constructor,
                name,
                named: flags & 1 != 0,
                public_name: flags & 2 != 0,
                optional: flags & 4 != 0,
                position: r.w()?,
            })
        })?;
        Some(ClassFact {
            key,
            library,
            extension,
            supers,
            written,
            members,
            fields,
            constructor_parameters,
        })
    })?;
    let function_variables = r.list(|r| Some((r.w()?, r.w()?)))?;
    let accessors = r.list(|r| Some((r.w()?, unopt(r.u()?)?, unopt(r.u()?)?)))?;
    let ambiguous = r.list(|r| r.w())?;
    let imports = r.list(|r| r.path())?;
    Some(FileRecord {
        path,
        uri,
        library,
        frozen: flags & 1 != 0,
        disk: flags & 2 != 0,
        stamp,
        length,
        modified,
        api,
        lines,
        wide,
        referenced,
        relations,
        declarations,
        patch_declarations,
        docs,
        classes,
        function_variables,
        accessors,
        ambiguous,
        imports,
    })
}

impl ReferenceIndex {
    /// The index as bytes (the child's answer, the disk cache): a header with
    /// the format, the root, the strings and the records.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.0.extend_from_slice(b"DFRI");
        w.u(u64::from(FORMAT));
        w.path(&self.root);
        for strings in [&self.keys, &self.words] {
            w.n(strings.texts.len());
            for t in &strings.texts {
                w.s(t);
            }
        }
        let records: Vec<&FileRecord> = self.records().collect();
        w.n(records.len());
        for r in records {
            write_record(&mut w, r);
        }
        let mut absent: Vec<(&PathBuf, &u64)> = self.absent.iter().collect();
        absent.sort();
        w.n(absent.len());
        for (p, s) in absent {
            w.path(p);
            w.u(*s);
        }
        w.0
    }

    /// The index of [`ReferenceIndex::encode`]'s bytes; `None` for another
    /// format or bad data.
    pub(crate) fn decode(bytes: &[u8]) -> Option<ReferenceIndex> {
        let rest = bytes.strip_prefix(b"DFRI")?;
        let mut r = Reader { bytes: rest, at: 0 };
        if r.w()? != FORMAT {
            return None;
        }
        let root = r.path()?;
        let mut index = ReferenceIndex::new(root);
        for i in 0..2 {
            let n = r.n()?;
            let strings = if i == 0 {
                &mut index.keys
            } else {
                &mut index.words
            };
            for _ in 0..n {
                let t = r.s()?;
                strings.id(t);
            }
            if strings.len() != n {
                return None;
            }
        }
        let records = r.list(read_record)?;
        index.put(records);
        let absent = r.list(|r| Some((r.path()?, r.u()?)))?;
        index.absent.extend(absent);
        Some(index)
    }
}

impl ReferenceIndex {
    /// Puts the records of `other` (another process's index, with its own
    /// strings) into this one, translating their keys and words.
    pub(crate) fn merge(&mut self, other: ReferenceIndex) {
        let keys: Vec<u32> = other.keys.texts.iter().map(|t| self.keys.id(t)).collect();
        let words: Vec<u32> = other.words.texts.iter().map(|t| self.words.id(t)).collect();
        let k = |v: &mut u32| {
            if *v != NONE {
                *v = keys[*v as usize];
            }
        };
        let w = |v: &mut u32| *v = words[*v as usize];
        let mut records = Vec::new();
        for mut r in other.files.into_iter().flatten() {
            k(&mut r.library);
            r.referenced.iter_mut().for_each(w);
            r.referenced.sort_unstable();
            for x in &mut r.relations {
                k(&mut x.key);
            }
            r.relations.sort_by_key(|x| (x.key, x.start, x.end, x.tag));
            for x in &mut r.declarations {
                k(&mut x.0);
            }
            r.patch_declarations.iter_mut().for_each(k);
            for d in &mut r.docs {
                w(&mut d.word);
                match &mut d.target {
                    DocTarget::Top { key, group } => {
                        k(key);
                        group.iter_mut().for_each(k);
                    }
                    DocTarget::Member { owner, name, .. } => {
                        k(owner);
                        w(name);
                    }
                    DocTarget::Constructor { key } => k(key),
                    DocTarget::Other => {}
                }
                match &mut d.concrete {
                    Some(ConcreteKey::Function(x) | ConcreteKey::Variable(x)) => k(x),
                    None => {}
                }
                if let Some(Some(c)) = &mut d.field_parameter {
                    k(c);
                }
            }
            for c in &mut r.classes {
                k(&mut c.key);
                k(&mut c.library);
                c.supers.iter_mut().for_each(k);
                c.written.iter_mut().for_each(k);
                for m in c.members.iter_mut().chain(c.fields.iter_mut()) {
                    w(&mut m.0);
                    k(&mut m.2);
                }
                for q in &mut c.constructor_parameters {
                    k(&mut q.constructor);
                    w(&mut q.name);
                }
            }
            for x in &mut r.function_variables {
                k(&mut x.0);
                k(&mut x.1);
            }
            for x in &mut r.accessors {
                k(&mut x.0);
                k(&mut x.1);
                k(&mut x.2);
            }
            r.ambiguous.iter_mut().for_each(k);
            records.push(r);
        }
        self.put(records);
        self.absent.extend(other.absent);
    }

    /// Notes the project files of `files` that have no record as absent.
    fn note_absent(&mut self, files: impl Iterator<Item = PathBuf>) {
        for f in files {
            let f = dartforge_elements::generated::key(&f);
            if self.by_path.contains_key(&f) {
                continue;
            }
            if let Ok(t) = std::fs::read_to_string(&f) {
                self.absent.insert(f, text_stamp(&t));
            }
        }
    }

    /// The paths of the records outside the SDK and the packages.
    pub(crate) fn project_paths(&self) -> Vec<PathBuf> {
        self.records()
            .filter(|r| !r.frozen)
            .map(|r| r.path.clone())
            .collect()
    }

    /// The project files whose library imports or exports, directly or not,
    /// the library of one of `changed` (the dependents of an API change).
    pub(crate) fn dependents(&self, changed: &[PathBuf]) -> Vec<PathBuf> {
        // Who imports each library (by the library of the imported file).
        let mut importers: HashMap<u32, Vec<u32>> = HashMap::new();
        for r in self.records().filter(|r| !r.frozen) {
            for i in &r.imports {
                if let Some(target) = self.file(i) {
                    importers.entry(target.library).or_default().push(r.library);
                }
            }
        }
        let mut seen: HashSet<u32> = HashSet::new();
        let mut stack: Vec<u32> = changed
            .iter()
            .filter_map(|p| self.file(p).map(|r| r.library))
            .collect();
        while let Some(lib) = stack.pop() {
            for &l in importers.get(&lib).into_iter().flatten() {
                if seen.insert(l) {
                    stack.push(l);
                }
            }
        }
        self.records()
            .filter(|r| !r.frozen && seen.contains(&r.library))
            .map(|r| r.path.clone())
            .collect()
    }
}

/// Re-indexes the libraries of `files` (project files, canonical keys) in a
/// load of the project of `uri` without the wide part, each library's bodies
/// inferred and dropped in turn. The records of their units come back, and
/// the files that are no unit of the load (a part without its library) are
/// left out.
pub(crate) fn build_files(
    sdk: &dartforge_elements::sdk::SdkLayout,
    documents: &crate::DocumentStore,
    uri: &str,
    files: &[PathBuf],
) -> Option<ReferenceIndex> {
    let file = crate::project::uri_file(uri)?;
    let root = crate::project::project_root(&file);
    let mut project = crate::project::load_project_bare(sdk, documents, uri)?;
    let wanted: HashSet<&Path> = files.iter().map(PathBuf::as_path).collect();
    let mut index = ReferenceIndex::new(root.clone());
    let libraries = project.program().libraries.len();
    for i in 0..libraries {
        let lib = LibraryId(i as u32);
        let units = project.program().library(lib).units.clone();
        let p = project.program();
        let hit = units.iter().any(|u| {
            p.unit(*u)
                .path
                .as_deref()
                .is_some_and(|c| wanted.contains(dartforge_elements::generated::key_of(c).as_ref()))
        });
        if !hit {
            continue;
        }
        project.query.infer_libraries(&[lib]);
        let records = Builder.library(&project, lib, &root, &mut index.keys, &mut index.words);
        for u in units {
            project.query.bodies.units[u.0 as usize] = Default::default();
            project.forget_unit(u);
        }
        index.put(records);
    }
    index.note_absent(files.iter().cloned());
    Some(index)
}
