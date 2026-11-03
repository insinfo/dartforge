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
    /// The id of each text, by the text's hash; a text whose hash another one
    /// already has goes to `collisions`.
    ids: HashMap<u64, u32>,
    collisions: HashMap<Box<str>, u32>,
    /// All the texts, one after the other, and where each one ends.
    text: String,
    ends: Vec<u32>,
}

fn string_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl Strings {
    /// The id of `s`, interning it.
    pub(crate) fn id(&mut self, s: &str) -> u32 {
        if let Some(i) = self.get(s) {
            return i;
        }
        let i = self.ends.len() as u32;
        self.text.push_str(s);
        self.ends.push(self.text.len() as u32);
        match self.ids.entry(string_hash(s)) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(i);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                self.collisions.insert(s.into(), i);
            }
        }
        i
    }

    /// The id of `s` when it is interned.
    pub(crate) fn get(&self, s: &str) -> Option<u32> {
        match self.ids.get(&string_hash(s)) {
            Some(&i) if self.text(i) == s => Some(i),
            Some(_) => self.collisions.get(s).copied(),
            None => None,
        }
    }

    fn text(&self, i: u32) -> &str {
        let end = self.ends[i as usize] as usize;
        let start = if i == 0 {
            0
        } else {
            self.ends[i as usize - 1] as usize
        };
        &self.text[start..end]
    }

    fn len(&self) -> usize {
        self.ends.len()
    }

    fn texts(&self) -> impl Iterator<Item = &str> {
        (0..self.ends.len() as u32).map(|i| self.text(i))
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// An extension type (its members redeclare, they do not override).
    pub extension_type: bool,
    /// Its mixins, in order.
    pub mixins: Vec<u32>,
    /// Its members by their name in the class's maps (`x`, `x_=`, `+`): name,
    /// static, function key and kind ([`Keys::function`]'s letter).
    pub raw_members: Vec<(u32, bool, u32, u8)>,
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
    /// A project file's identifiers: the start of each one's word and the
    /// [`IdentFact`] of what it denotes (an index of `facts`), sorted.
    pub idents: Vec<(u32, u32)>,
    pub facts: Vec<IdentFact>,
    /// A project file's `prepareRename` answers: the start of each
    /// identifier's word and the answer (an index of `prepare_facts`), sorted.
    pub prepares: Vec<(u32, u32)>,
    pub prepare_facts: Vec<PrepareFact>,
}

/// What `textDocument/prepareRename` answers at an identifier
/// ([`crate::rename::prepare_in`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum PrepareFact {
    /// Nothing to rename there.
    Nothing,
    /// The range and the current name.
    Range { start: u32, end: u32, old: String },
    /// The rename is refused, with this message.
    Refused(String),
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
            Some(u)
                if self.project.patch_keys
                    && self.project.program().unit(u).role == UnitRole::Patch =>
            {
                "!p"
            }
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
        // The implicit setter has the getter's name: `j`, the getter `i`.
        let kind = function_kind_letter(fe.kind, p, f) as char;
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

/// The letter of a function's kind in its key.
fn function_kind_letter(
    kind: FunctionKind,
    p: &dartforge_elements::model::Program,
    f: FunctionElementId,
) -> u8 {
    match kind {
        FunctionKind::Function => b'f',
        FunctionKind::Getter => b'g',
        FunctionKind::Setter => b's',
        FunctionKind::Operator => b'o',
        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => b'k',
        FunctionKind::ImplicitAccessor => {
            let fe = p.function(f);
            if fe.variable.is_some_and(|v| p.variable(v).setter == Some(f)) {
                b'j'
            } else {
                b'i'
            }
        }
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
        tables: &mut Tables<'_>,
    ) -> Vec<FileRecord> {
        let mut emit = Emit {
            project,
            keys: Keys::new(project),
        };
        let by_unit = declared_by_unit(project, lib);
        let mut records = emit.library(&by_unit, lib, root, tables.keys, tables.words);
        for r in records.iter_mut().filter(|r| !r.frozen) {
            if let Some(&u) = project.program().library(lib).units.iter().find(|u| {
                project
                    .program()
                    .unit(**u)
                    .path
                    .as_deref()
                    .is_some_and(|c| dartforge_elements::generated::key_of(c).as_ref() == r.path)
            }) {
                identifier_facts(project, u, r, tables);
            }
        }
        records
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
        let mut raw_members = Vec::new();
        for (map, is_static) in [(&cl.instance_members, false), (&cl.static_members, true)] {
            let mut list: Vec<(&dartforge_intern::SymbolId, &FunctionElementId)> =
                map.iter().collect();
            list.sort_by_key(|(_, f)| **f);
            for (sym, f) in list {
                let key = self.keys.function(*f);
                raw_members.push((
                    words.id(project.name(*sym)),
                    is_static,
                    keys.id(&key),
                    function_kind_letter(p.function(*f).kind, p, *f),
                ));
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
            extension_type: cl.decl.is_some_and(|dr| {
                matches!(
                    p.unit(dr.unit).ast.decl(dr.decl).kind,
                    ast::DeclKind::ExtensionType(_)
                )
            }),
            mixins: cl
                .mixin_classes
                .iter()
                .map(|m| keys.id(&self.keys.class(*m)))
                .collect(),
            raw_members,
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

/// The identifier facts of project unit `u` (its record `r`): what
/// [`Project::identify`] says each identifier denotes, as an [`IndexTarget`];
/// for a target only the file has, the whole answer of the search (when it
/// stays in the file). A parameter's search reaches other files, and an
/// identifier without a fact is identified at the query.
fn identifier_facts(project: &Project, u: UnitId, r: &mut FileRecord, tables: &mut Tables<'_>) {
    let unit = project.program().unit(u);
    let source = &unit.source;
    let b = source.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut facts: HashMap<IdentFact, u32> = HashMap::new();
    let mut locals: HashMap<String, Option<u32>> = HashMap::new();
    let mut idents = Vec::new();
    let mut prepare_facts: HashMap<PrepareFact, u32> = HashMap::new();
    let mut prepares = Vec::new();
    let cx = crate::refactorings::Context::new(project, u);
    // The identifier tokens and the parts of the documentation references: an
    // identifier elsewhere (a comment, a string) has no fact, and the query
    // identifies it in the library as before.
    let mut positions: Vec<usize> = match dartforge_frontend::lexer::lex(source) {
        Ok(tokens) => tokens
            .iter()
            .filter(|t| t.kind == dartforge_frontend::token::Kind::Ident)
            .map(|t| t.span.start)
            .collect(),
        Err(_) => Vec::new(),
    };
    if source.contains('[') {
        for c in project.unit_comments(u).iter() {
            for r in &c.references {
                positions.extend(r.iter().map(|(s, _)| s.start));
            }
        }
    }
    positions.sort_unstable();
    positions.dedup();
    for o in positions {
        if o >= b.len() || !ident(b[o]) || b[o].is_ascii_digit() || (o > 0 && ident(b[o - 1])) {
            continue;
        }
        let Ok(Some(d)) = project.identify(u, o) else {
            continue;
        };
        let prepare = match crate::rename::prepare_with(project, &cx, u, o) {
            Ok(None) => PrepareFact::Nothing,
            Ok(Some((s, old))) => PrepareFact::Range {
                start: s.start as u32,
                end: s.end as u32,
                old,
            },
            Err(message) => PrepareFact::Refused(message),
        };
        let n = prepare_facts.len() as u32;
        prepares.push((o as u32, *prepare_facts.entry(prepare).or_insert(n)));
        let fact = match target_of(project, u, tables, &d) {
            Some(t) => Some(IdentFact::Target(t)),
            None => {
                // A named parameter's search reaches the labels of other files and
                // the overrides' homonyms: it has no fact. An optional positional
                // one cites the start of its argument in the calls of other files,
                // whose bodies are not inferred while this library is recorded: the
                // fact names those relations, read at the query.
                let mut positional = None;
                if let Target::Local { declaration, .. } = d.target
                    && let Some((q, _)) = crate::project::parameter_in(&unit.ast, declaration)
                {
                    if q.kind == ast::ParameterKind::Named {
                        continue;
                    }
                    if let Some((f, i)) = project.optional_parameter_of(u, declaration) {
                        let key = format!("{}/a{i}", Keys::new(project).function(f));
                        positional = Some(tables.keys.id(&key));
                    }
                }
                // A prefix's answer depends on where its use is (§11.5): in an
                // expression, in a type, or the `as p` of its directive.
                let label = match d.target {
                    Target::Prefix { .. } => {
                        let in_directive = unit
                            .unit
                            .directives
                            .iter()
                            .any(|x| x.span.start <= o && o < x.span.end);
                        format!("{:?} {} {in_directive}", d.target, d.expr.is_some())
                    }
                    _ => format!("{:?}", d.target),
                };
                if let Some(known) = locals.get(&label) {
                    if let Some(i) = known {
                        idents.push((o as u32, *i));
                    }
                    continue;
                }
                let answer = crate::semantic::references_at(project, u, o).and_then(|a| {
                    let here = |x: &(String, Span)| x.0 == r.uri;
                    (a.declaration.as_ref().is_none_or(here) && a.uses.iter().all(here)).then(
                        || IdentFact::Local {
                            declaration: a.declaration.map(|(_, s)| (s.start as u32, s.end as u32)),
                            uses: a
                                .uses
                                .iter()
                                .map(|(_, s)| (s.start as u32, s.end as u32))
                                .collect(),
                            positional,
                        },
                    )
                });
                let i = answer.map(|f| {
                    let n = facts.len() as u32;
                    *facts.entry(f).or_insert(n)
                });
                locals.insert(label, i);
                if let Some(i) = i {
                    idents.push((o as u32, i));
                }
                continue;
            }
        };
        if let Some(f) = fact {
            let n = facts.len() as u32;
            let i = *facts.entry(f).or_insert(n);
            idents.push((o as u32, i));
        }
    }
    let mut ordered: Vec<(u32, IdentFact)> = facts.into_iter().map(|(f, i)| (i, f)).collect();
    ordered.sort_by_key(|x| x.0);
    r.facts = ordered.into_iter().map(|(_, f)| f).collect();
    r.idents = idents;
    let mut ordered: Vec<(u32, PrepareFact)> =
        prepare_facts.into_iter().map(|(f, i)| (i, f)).collect();
    ordered.sort_by_key(|x| x.0);
    r.prepare_facts = ordered.into_iter().map(|(_, f)| f).collect();
    r.prepares = prepares;
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

impl Derived {
    /// Adds what record `r`, in slot `i`, contributes (the full build, file
    /// by file in slot order).
    fn add(&mut self, i: usize, r: &FileRecord) {
        for w in &r.referenced {
            self.by_word.entry(*w).or_default().push(i);
        }
        self.by_library.entry(r.library).or_default().push(i);
        for &(k, s, e) in &r.declarations {
            self.declarations.entry(k).or_insert((i, s, e));
        }
        for &k in &r.patch_declarations {
            self.patch_declarations.entry(k).or_insert(r.library);
        }
        for (j, c) in r.classes.iter().enumerate() {
            self.classes.entry(c.key).or_insert((i, j));
            for &s in &c.written {
                let v = self.subtypes.entry(s).or_default();
                if !v.contains(&c.key) {
                    v.push(c.key);
                }
            }
        }
        for &(f, v) in &r.function_variables {
            self.function_variables.insert(f, v);
        }
        for &(v, g, s) in &r.accessors {
            self.accessors.insert(v, (g, s));
        }
        self.ambiguous.extend(r.ambiguous.iter().copied());
    }

    /// Replaces in place what `old`, in slot `i`, contributed with what `new`
    /// contributes, giving what a full build over the new records gives;
    /// `false` when that is not certain without one (the caller then builds
    /// again): `new` declares other keys than `old` with other facts, which
    /// another file may also declare.
    fn replace(&mut self, i: usize, old: &FileRecord, new: &FileRecord) -> bool {
        let keys = |r: &FileRecord| -> Vec<u32> {
            let mut v: Vec<u32> = r.declarations.iter().map(|d| d.0).collect();
            v.sort_unstable();
            v
        };
        let classes = |r: &FileRecord| -> Vec<(u32, Vec<u32>)> {
            r.classes
                .iter()
                .map(|c| (c.key, c.written.clone()))
                .collect()
        };
        if old.library != new.library
            || keys(old) != keys(new)
            || classes(old) != classes(new)
            || old.function_variables != new.function_variables
            || old.accessors != new.accessors
            || old.ambiguous != new.ambiguous
            || old.patch_declarations != new.patch_declarations
        {
            return false;
        }
        // The words: each list keeps the slots in order.
        let place = |v: &mut Vec<usize>| {
            if let Err(at) = v.binary_search(&i) {
                v.insert(at, i);
            }
        };
        let old_words: HashSet<u32> = old.referenced.iter().copied().collect();
        let new_words: HashSet<u32> = new.referenced.iter().copied().collect();
        for w in old_words.difference(&new_words) {
            if let Some(v) = self.by_word.get_mut(w) {
                v.retain(|&x| x != i);
                if v.is_empty() {
                    self.by_word.remove(w);
                }
            }
        }
        for w in new_words.difference(&old_words) {
            place(self.by_word.entry(*w).or_default());
        }
        // The same keys: only the spans of the declarations this slot holds
        // move.
        let mut first: HashSet<u32> = HashSet::new();
        for &(k, s, e) in &new.declarations {
            if first.insert(k)
                && let Some(d) = self.declarations.get_mut(&k)
                && d.0 == i
            {
                *d = (i, s, e);
            }
        }
        true
    }
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
    /// The files the identifier facts name (canonical paths).
    pub(crate) paths: Strings,
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
                Some(&i) => {
                    // A record of the same file with the same keys (a body edit)
                    // updates what is derived in place; anything else builds it
                    // again at the next search.
                    let kept = match (&mut self.derived, &self.files[i]) {
                        (Some(d), Some(old)) => d.replace(i, old, &r),
                        _ => false,
                    };
                    if !kept {
                        self.derived = None;
                    }
                    self.files[i] = Some(r);
                }
                None => {
                    let i = self.files.len();
                    if let Some(d) = &mut self.derived {
                        d.add(i, &r);
                    }
                    self.by_path.insert(r.path.clone(), i);
                    self.files.push(Some(r));
                }
            }
        }
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
    /// The bytes each part of the index holds (capacity-based, without the
    /// allocator's overhead), for the memory log.
    pub(crate) fn census(&self) -> Vec<(&'static str, usize)> {
        use std::mem::size_of;
        let v = |n: usize, s: usize| n * s;
        let mut parts: Vec<(&'static str, usize)> = Vec::new();
        let mut add = |name: &'static str, b: usize| match parts.iter_mut().find(|p| p.0 == name) {
            Some(p) => p.1 += b,
            None => parts.push((name, b)),
        };
        for r in self.records() {
            add("records", size_of::<FileRecord>());
            add(
                "relations",
                v(r.relations.capacity(), size_of::<Relation>()),
            );
            add("lines", v(r.lines.capacity(), 4) + v(r.wide.capacity(), 8));
            add("referenced", v(r.referenced.capacity(), 4));
            add("declarations", v(r.declarations.capacity(), 12));
            add("docs", v(r.docs.capacity(), size_of::<DocFact>()));
            add(
                "classes",
                r.classes
                    .iter()
                    .map(|c| {
                        size_of::<ClassFact>()
                            + 4 * (c.supers.capacity() + c.written.capacity() + c.mixins.capacity())
                            + 12 * (c.members.capacity() + c.fields.capacity())
                            + 16 * c.raw_members.capacity()
                            + size_of::<ConstructorParameter>()
                                * c.constructor_parameters.capacity()
                    })
                    .sum(),
            );
            add(
                "idents",
                v(r.idents.capacity(), 8) + v(r.prepares.capacity(), 8),
            );
            add(
                "facts",
                r.facts.capacity() * size_of::<IdentFact>()
                    + r.prepare_facts.capacity() * size_of::<PrepareFact>(),
            );
            add(
                "paths and imports",
                r.path.capacity()
                    + r.uri.capacity()
                    + r.imports.iter().map(|i| i.capacity() + 24).sum::<usize>(),
            );
        }
        for (name, t) in [
            ("keys", &self.keys),
            ("words", &self.words),
            ("paths", &self.paths),
        ] {
            add(
                name,
                t.text.capacity() + t.ends.capacity() * 4 + t.ids.capacity() * 16,
            );
        }
        parts
    }

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
                d.add(i, r);
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

    /// The references of `target` over the records, as
    /// `semantic::references_in` gives them over the wide load: the
    /// declaration and the uses, each with its URI, byte span and LSP range.
    /// `Err` when the index does not answer. The project's records must be of
    /// the current texts ([`crate::semantic`] refreshes them first).
    pub(crate) fn references(
        &mut self,
        target: &IndexTarget,
    ) -> Result<(Option<Located>, Vec<Located>), Miss> {
        self.ensure_derived();
        let this: &Self = self;
        this.search(target)
    }

    /// The record of the file at `path` (a [`Strings`] id of [`Self::paths`]).
    fn file_index(&self, path: u32) -> Option<usize> {
        if path == NONE {
            return None;
        }
        self.by_path.get(Path::new(self.paths.text(path))).copied()
    }

    /// `path`'s URI, span and, when the file has a record, LSP range.
    fn locate(&self, path: u32, start: u32, end: u32) -> Option<Located> {
        let span = Span {
            start: start as usize,
            end: end as usize,
        };
        if let Some(r) = self.file_index(path).and_then(|f| self.files[f].as_ref()) {
            let (l0, c0) = r.position(start);
            let (l1, c1) = r.position(end);
            return Some((r.uri.clone(), span, Some((l0, c0, l1, c1))));
        }
        let uri = url::Url::from_file_path(self.paths.text(path))
            .ok()?
            .to_string();
        Some((uri, span, None))
    }

    fn search(&self, target: &IndexTarget) -> Result<(Option<Located>, Vec<Located>), Miss> {
        let d = self.derived.as_ref().expect("ensured by the caller");
        let name = self.words.text(target.name).to_string();
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
        let covers = match &target.kind {
            TargetKey::Top(_) => Covers::Top(target.group.iter().copied().collect()),
            TargetKey::Member {
                owner,
                name: member,
                is_static,
            } => {
                let member = self.words.text(*member);
                let family = self
                    .family(d, *owner, member, *is_static)
                    .ok_or(Miss::Unsupported("a class without facts"))?;
                let parameters = if *is_static {
                    Vec::new()
                } else {
                    self.field_parameter_relations(d, &family, *owner, member)
                };
                Covers::Member {
                    family,
                    owner: *owner,
                    is_static: *is_static,
                    parameters,
                }
            }
            TargetKey::Constructor(k) => Covers::Constructor(*k),
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
        // The declarations, which are not uses, and the declaring files and
        // libraries (`None`: an SDK patch, which has no uses).
        let mut declarations: HashSet<(usize, u32, u32)> = HashSet::new();
        let mut declaration_files: Vec<(Option<usize>, u32)> = Vec::new();
        match &covers {
            Covers::Top(_) | Covers::Constructor(_) => {
                for &(path, s, e, library) in &target.declarations {
                    let f = self.file_index(path);
                    if path != NONE && f.is_none() {
                        return Err(Miss::Key("a declaration outside the index".to_string()));
                    }
                    if let Some(f) = f {
                        declarations.insert((f, s, e));
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
        if let Some((path, library)) = target.declaration {
            let f = self.file_index(path);
            if path != NONE && f.is_none() {
                return Err(Miss::Key("a declaration outside the index".to_string()));
            }
            declaration_files.push((f, library));
        }
        // The candidate files of `_addResults`.
        let private = name.starts_with('_');
        let name_word = Some(target.name);
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
        let word_id = target.word;
        let target_key = match &target.kind {
            TargetKey::Top(k) => Some(*k),
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
                if let Some(w) = word_id
                    && w != doc.word
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
                    // Only a member of the field's own class cites it.
                    let own = match &covers {
                        Covers::Member { owner, .. } => {
                            self.class_fact(d, *owner).is_some_and(|c| !c.extension)
                                && class == Some(*owner)
                        }
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
            .filter_map(|(f, s, e)| {
                let r = self.files[f].as_ref()?;
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
            })
            .collect();
        located_uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        let declaration = target
            .answer
            .and_then(|(path, s, e)| self.locate(path, s, e));
        Ok((declaration, located_uses))
    }

    /// The implementations of `target` ([`Project::implementations`]): its
    /// class's subtypes, or the member each of them declares, over the class
    /// facts. `None` when the index cannot tell (a class without facts, a
    /// declaration in an SDK patch).
    pub(crate) fn implementations(&mut self, target: &IndexTarget) -> Option<Vec<(String, Span)>> {
        self.ensure_derived();
        let this: &Self = self;
        this.walk_implementations(target.pivot.as_ref()?)
    }

    fn walk_implementations(&self, pv: &KeyPivot) -> Option<Vec<(String, Span)>> {
        use crate::hierarchy::PivotKind;
        let d = self.derived.as_ref().expect("ensured by the caller");
        let fact = |c: u32| self.class_fact(d, c);
        // `index_all_subtypes`: the transitive written subtypes.
        let mut seen: HashSet<u32> = HashSet::new();
        let mut order = Vec::new();
        let mut stack = vec![pv.class];
        while let Some(x) = stack.pop() {
            for &s in d.subtypes.get(&x).map(|v| &v[..]).unwrap_or(&[]) {
                if s != pv.class && seen.insert(s) {
                    order.push(s);
                    stack.push(s);
                }
            }
        }
        let declared = |k: u32| -> Option<(usize, u32, u32)> { d.declarations.get(&k).copied() };
        let name = pv.member.map(|(n, _)| self.words.text(n).to_string());
        let private = name.as_deref().is_some_and(|n| n.starts_with('_'));
        let setter_name = name.as_deref().map(|n| format!("{n}_="));
        // `implementation_member`'s `find`: the member of the pivot's kind.
        let find = |c: &ClassFact| -> Option<u32> {
            let lookup = |key: &str, accepts: &dyn Fn(u8) -> bool| -> Option<u32> {
                let w = self.words.get(key)?;
                for is_static in [false, true] {
                    if let Some(&(_, _, f, kind)) = c
                        .raw_members
                        .iter()
                        .find(|(n, s, _, _)| *n == w && *s == is_static)
                        && accepts(kind)
                    {
                        return Some(f);
                    }
                }
                None
            };
            let name = name.as_deref()?;
            let method = || lookup(name, &|k| k == b'f' || k == b'o');
            let getter = || lookup(name, &|k| k == b'g' || k == b'i');
            let setter = || {
                lookup(setter_name.as_deref()?, &|k| {
                    k == b's' || k == b'i' || k == b'j'
                })
            };
            match pv.kind {
                PivotKind::Method => method(),
                PivotKind::Getter => getter(),
                PivotKind::Setter => setter(),
                PivotKind::Field { final_ } => {
                    getter().or_else(|| if final_ { None } else { setter() })
                }
            }
        };
        let accessible = |c: &ClassFact| !private || c.library == pv.library;
        let mut output = Vec::new();
        let mut places: HashSet<(usize, u32)> = HashSet::new();
        for x in order {
            let fx = fact(x)?;
            let local = match pv.member {
                Some(_)
                    if pv.needs_member && (fact(pv.class)?.extension_type || fx.extension_type) =>
                {
                    None
                }
                Some(_) if pv.needs_member => {
                    let mut found = None;
                    if let Some(f) = find(fx).filter(|_| accessible(fx)) {
                        found = Some(Some(f));
                    } else {
                        for &m in fx.mixins.iter().rev() {
                            let fm = fact(m)?;
                            if let Some(f) = find(fm).filter(|_| accessible(fm)) {
                                found = Some((f != pv.pivot).then_some(f));
                                break;
                            }
                        }
                    }
                    match found {
                        Some(Some(f)) => Some(declared(f)?),
                        _ => None,
                    }
                }
                _ => Some(declared(x)?),
            };
            if let Some((f, s, e)) = local
                && places.insert((f, s))
            {
                let r = self.files[f].as_ref()?;
                output.push((
                    r.uri.clone(),
                    Span {
                        start: s as usize,
                        end: e as usize,
                    },
                ));
            }
        }
        Some(output)
    }

    /// What the record of the open document `path` says the identifier at
    /// `start` (the start of its word) denotes; `None` when the index has no
    /// answer for it (the caller identifies it in the document's library).
    pub(crate) fn fact_at(&self, path: &Path, start: usize) -> Option<&IdentFact> {
        let r = self.file(path)?;
        let i = r
            .idents
            .binary_search_by_key(&(start as u32), |x| x.0)
            .ok()?;
        r.facts.get(r.idents[i].1 as usize)
    }

    /// What the record of the open document `path` says `prepareRename`
    /// answers at the word starting at `start`.
    pub(crate) fn prepare_at(&self, path: &Path, start: usize) -> Option<&PrepareFact> {
        let r = self.file(path)?;
        let i = r
            .prepares
            .binary_search_by_key(&(start as u32), |x| x.0)
            .ok()?;
        r.prepare_facts.get(r.prepares[i].1 as usize)
    }

    /// [`ReferenceIndex::references`] of the answer a record keeps for a
    /// local target of the file at `path`.
    pub(crate) fn local_answer(
        &self,
        path: &Path,
        declaration: Option<(u32, u32)>,
        uses: &[(u32, u32)],
        positional: Option<u32>,
    ) -> Option<(Option<Located>, Vec<Located>)> {
        let here = *self.by_path.get(path)?;
        let at = |f: usize, (s, e): (u32, u32)| -> Option<Located> {
            let r = self.files[f].as_ref()?;
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
        let mut found: Vec<(usize, u32, u32)> = uses.iter().map(|&(s, e)| (here, s, e)).collect();
        // The optional positional parameter's arguments, in every file.
        if let Some(k) = positional {
            for (f, r) in self.files.iter().enumerate() {
                let Some(r) = r else { continue };
                for rel in r.relations_of(k) {
                    if rel.tag == Tag::Positional {
                        found.push((f, rel.start, rel.end));
                    }
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        if let Some(d) = declaration {
            found.retain(|x| *x != (here, d.0, d.1));
        }
        let mut located: Vec<Located> = found
            .into_iter()
            .filter_map(|(f, s, e)| at(f, (s, e)))
            .collect();
        located.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        Some((declaration.and_then(|d| at(here, d)), located))
    }
}

/// What a search looks for, in the index's terms: the target, its concrete
/// element, and what the search takes from the document's program
/// ([`target_of`]): the names, the declarations and the declaring library.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct IndexTarget {
    pub kind: TargetKey,
    pub concrete: Option<ConcreteKey>,
    /// [`Project::search_name`] (a word).
    pub name: u32,
    /// [`Project::target_word`] (a word).
    pub word: Option<u32>,
    /// A top-level target's [`Project::top_elements`].
    pub group: Vec<u32>,
    /// A top-level or constructor target's [`Project::declarations`]: the
    /// file ([`ReferenceIndex::paths`], [`NONE`] for an SDK patch), the span
    /// and the library.
    pub declarations: Vec<(u32, u32, u32, u32)>,
    /// [`Project::declaration`]: its file ([`NONE`] for a patch) and library.
    pub declaration: Option<(u32, u32)>,
    /// [`Project::declaration_to_references`]: file, start and end.
    pub answer: Option<(u32, u32, u32)>,
    /// [`Project::implementation_pivot`], by key.
    pub pivot: Option<KeyPivot>,
}

/// [`crate::hierarchy::ImplementationPivot`] by key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct KeyPivot {
    pub class: u32,
    /// The member's base name (a word) and staticness.
    pub member: Option<(u32, bool)>,
    pub kind: crate::hierarchy::PivotKind,
    /// The pivot function's key ([`NONE`]: none).
    pub pivot: u32,
    /// The pivot's library (its URI, a key string).
    pub library: u32,
    pub needs_member: bool,
}

/// The target of a search, by key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TargetKey {
    Top(u32),
    Member {
        owner: u32,
        name: u32,
        is_static: bool,
    },
    Constructor(u32),
}

/// What a record says an identifier of its file denotes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum IdentFact {
    /// A target the index searches.
    Target(IndexTarget),
    /// A target only this file has (a local, a type parameter, a prefix): the
    /// whole answer, declaration and uses. An optional positional parameter
    /// also cites the start of its argument in the calls of every file: the
    /// key of those relations (`<function>/a<i>`).
    Local {
        declaration: Option<(u32, u32)>,
        uses: Vec<(u32, u32)>,
        positional: Option<u32>,
    },
}

/// The strings an [`IndexTarget`] is made of.
pub(crate) struct Tables<'a> {
    pub keys: &'a mut Strings,
    pub words: &'a mut Strings,
    pub paths: &'a mut Strings,
}

/// The [`IndexTarget`] of `d` in `project`'s program: `None` for a target the
/// index does not search (a local, a type parameter, a prefix).
pub(crate) fn target_of(
    project: &Project,
    unit: UnitId,
    t: &mut Tables<'_>,
    d: &Denoted,
) -> Option<IndexTarget> {
    let mut keys = Keys::new(project);
    let p = project.program();
    let kind = match &d.target {
        Target::Top(el) => TargetKey::Top(t.keys.id(&keys.element(*el)?)),
        Target::Member {
            owner,
            name,
            is_static,
        } => {
            let owner = match owner {
                Owner::Class(c) => keys.class(*c),
                Owner::Extension(x) => keys.extension(*x),
            };
            TargetKey::Member {
                owner: t.keys.id(&owner),
                name: t.words.id(name),
                is_static: *is_static,
            }
        }
        Target::Constructor(f) => TargetKey::Constructor(t.keys.id(&keys.function(*f))),
        _ => return None,
    };
    let name = t.words.id(&project.search_name(&d.target)?);
    let word = project.target_word(&d.target).map(|w| t.words.id(&w));
    let group = match &d.target {
        Target::Top(el) => project
            .top_elements(*el)
            .into_iter()
            .filter_map(|e| keys.element(e))
            .map(|k| t.keys.id(&k))
            .collect(),
        _ => Vec::new(),
    };
    let path_of = |u: UnitId, paths: &mut Strings| -> u32 {
        let unit = p.unit(u);
        match unit.path.as_deref() {
            Some(path) if unit.role != UnitRole::Patch => {
                paths.id(&dartforge_elements::generated::key_of(path).to_string_lossy())
            }
            _ => NONE,
        }
    };
    let library_of = |u: UnitId, keys: &mut Strings| keys.id(&p.library(p.unit(u).library).uri);
    let declarations = match &d.target {
        Target::Top(_) | Target::Constructor(_) => project
            .declarations(&d.target)
            .into_iter()
            .map(|(u, s, e)| {
                (
                    path_of(u, t.paths),
                    s as u32,
                    e as u32,
                    library_of(u, t.keys),
                )
            })
            .collect(),
        _ => Vec::new(),
    };
    let declaration = project
        .declaration(d)
        .map(|(u, _)| (path_of(u, t.paths), library_of(u, t.keys)));
    let answer = project.declaration_to_references(d).and_then(|(u, s)| {
        let path = p.unit(u).path.as_deref()?;
        let path = t
            .paths
            .id(&dartforge_elements::generated::key_of(path).to_string_lossy());
        Some((path, s.start as u32, s.end as u32))
    });
    let concrete = match d.concrete {
        Some(Concrete::Function(f)) => Some(ConcreteKey::Function(t.keys.id(&keys.function(f)))),
        Some(Concrete::Variable(v)) => Some(ConcreteKey::Variable(t.keys.id(&keys.variable(v)))),
        None => None,
    };
    let pivot = project.implementation_pivot(unit, d).map(|v| KeyPivot {
        class: t.keys.id(&keys.class(v.class)),
        member: v.member.as_ref().map(|(n, s)| (t.words.id(n), *s)),
        kind: v.kind,
        pivot: v.pivot.map_or(NONE, |f| t.keys.id(&keys.function(f))),
        library: t.keys.id(&p.library(v.library).uri),
        needs_member: v.needs_member,
    });
    Some(IndexTarget {
        kind,
        concrete,
        name,
        word,
        group,
        declarations,
        declaration,
        answer,
        pivot,
    })
}

/// The parent process of an index child when it started
/// ([`watch_parent`]).
#[cfg(unix)]
static PARENT: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// Remembers the parent of this (child) process: [`orphaned`] then tells when
/// the server that started it is gone.
pub(crate) fn watch_parent() {
    #[cfg(unix)]
    let _ = PARENT.set(std::os::unix::process::parent_id());
}

/// The server that started this child is gone (Unix; never elsewhere): the
/// build stops instead of finishing for nobody.
fn orphaned() -> bool {
    #[cfg(unix)]
    {
        PARENT
            .get()
            .is_some_and(|p| *p != std::os::unix::process::parent_id())
    }
    #[cfg(not(unix))]
    {
        false
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
        if orphaned() {
            std::process::exit(3);
        }
        project.query.infer_libraries(&[lib]);
        let mut tables = Tables {
            keys: &mut index.keys,
            words: &mut index.words,
            paths: &mut index.paths,
        };
        let records = builder.library(&project, lib, &root, &mut tables);
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
const FORMAT: u32 = 6;

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
        w.u(u64::from(c.extension_type));
        w.n(c.mixins.len());
        for &m in &c.mixins {
            w.u(u64::from(m));
        }
        w.n(c.raw_members.len());
        for &(n, s, f, k) in &c.raw_members {
            w.u(u64::from(n));
            w.u(u64::from(s));
            w.u(u64::from(f));
            w.u(u64::from(k));
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
    let mut last = 0;
    w.n(r.idents.len());
    for &(o, f) in &r.idents {
        w.u(u64::from(o - last));
        last = o;
        w.u(u64::from(f));
    }
    w.n(r.facts.len());
    for f in &r.facts {
        write_fact(w, f);
    }
    let mut last = 0;
    w.n(r.prepares.len());
    for &(o, f) in &r.prepares {
        w.u(u64::from(o - last));
        last = o;
        w.u(u64::from(f));
    }
    w.n(r.prepare_facts.len());
    for f in &r.prepare_facts {
        match f {
            PrepareFact::Nothing => w.u(0),
            PrepareFact::Range { start, end, old } => {
                w.u(1);
                w.u(u64::from(*start));
                w.u(u64::from(end - start));
                w.s(old);
            }
            PrepareFact::Refused(m) => {
                w.u(2);
                w.s(m);
            }
        }
    }
}

fn write_fact(w: &mut Writer, f: &IdentFact) {
    match f {
        IdentFact::Local {
            declaration,
            uses,
            positional,
        } => {
            w.u(1);
            w.u(opt(positional.unwrap_or(NONE)));
            match declaration {
                None => w.u(0),
                Some((s, e)) => {
                    w.u(1);
                    w.u(u64::from(*s));
                    w.u(u64::from(e - s));
                }
            }
            w.n(uses.len());
            for &(s, e) in uses {
                w.u(u64::from(s));
                w.u(u64::from(e - s));
            }
        }
        IdentFact::Target(t) => {
            w.u(0);
            match &t.kind {
                TargetKey::Top(k) => {
                    w.u(0);
                    w.u(u64::from(*k));
                }
                TargetKey::Member {
                    owner,
                    name,
                    is_static,
                } => {
                    w.u(1);
                    w.u(u64::from(*owner));
                    w.u(u64::from(*name));
                    w.u(u64::from(*is_static));
                }
                TargetKey::Constructor(k) => {
                    w.u(2);
                    w.u(u64::from(*k));
                }
            }
            match t.concrete {
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
            w.u(u64::from(t.name));
            w.u(t.word.map_or(0, |x| u64::from(x) + 1));
            w.n(t.group.len());
            for &g in &t.group {
                w.u(u64::from(g));
            }
            w.n(t.declarations.len());
            for &(path, s, e, library) in &t.declarations {
                w.u(opt(path));
                w.u(u64::from(s));
                w.u(u64::from(e - s));
                w.u(u64::from(library));
            }
            match t.declaration {
                None => w.u(0),
                Some((path, library)) => {
                    w.u(1);
                    w.u(opt(path));
                    w.u(u64::from(library));
                }
            }
            match t.answer {
                None => w.u(0),
                Some((path, s, e)) => {
                    w.u(1);
                    w.u(u64::from(path));
                    w.u(u64::from(s));
                    w.u(u64::from(e - s));
                }
            }
            match &t.pivot {
                None => w.u(0),
                Some(v) => {
                    use crate::hierarchy::PivotKind;
                    w.u(1);
                    w.u(u64::from(v.class));
                    match v.member {
                        None => w.u(0),
                        Some((n, s)) => {
                            w.u(1);
                            w.u(u64::from(n));
                            w.u(u64::from(s));
                        }
                    }
                    w.u(match v.kind {
                        PivotKind::Method => 0,
                        PivotKind::Getter => 1,
                        PivotKind::Setter => 2,
                        PivotKind::Field { final_: false } => 3,
                        PivotKind::Field { final_: true } => 4,
                    });
                    w.u(opt(v.pivot));
                    w.u(u64::from(v.library));
                    w.u(u64::from(v.needs_member));
                }
            }
        }
    }
}

fn read_fact(r: &mut Reader<'_>) -> Option<IdentFact> {
    Some(match r.u()? {
        1 => {
            let positional = match unopt(r.u()?)? {
                NONE => None,
                k => Some(k),
            };
            let declaration = match r.u()? {
                0 => None,
                _ => {
                    let s = r.w()?;
                    Some((s, s.checked_add(r.w()?)?))
                }
            };
            let uses = r.list(|r| {
                let s = r.w()?;
                Some((s, s.checked_add(r.w()?)?))
            })?;
            IdentFact::Local {
                declaration,
                uses,
                positional,
            }
        }
        0 => {
            let kind = match r.u()? {
                0 => TargetKey::Top(r.w()?),
                1 => TargetKey::Member {
                    owner: r.w()?,
                    name: r.w()?,
                    is_static: r.u()? != 0,
                },
                2 => TargetKey::Constructor(r.w()?),
                _ => return None,
            };
            let concrete = match r.u()? {
                0 => None,
                1 => Some(ConcreteKey::Function(r.w()?)),
                2 => Some(ConcreteKey::Variable(r.w()?)),
                _ => return None,
            };
            let name = r.w()?;
            let word = match r.u()? {
                0 => None,
                x => Some(u32::try_from(x - 1).ok()?),
            };
            let group = r.list(|r| r.w())?;
            let declarations = r.list(|r| {
                let path = unopt(r.u()?)?;
                let s = r.w()?;
                let e = s.checked_add(r.w()?)?;
                Some((path, s, e, r.w()?))
            })?;
            let declaration = match r.u()? {
                0 => None,
                _ => Some((unopt(r.u()?)?, r.w()?)),
            };
            let answer = match r.u()? {
                0 => None,
                _ => {
                    let path = r.w()?;
                    let s = r.w()?;
                    Some((path, s, s.checked_add(r.w()?)?))
                }
            };
            let pivot = match r.u()? {
                0 => None,
                _ => {
                    use crate::hierarchy::PivotKind;
                    let class = r.w()?;
                    let member = match r.u()? {
                        0 => None,
                        _ => Some((r.w()?, r.u()? != 0)),
                    };
                    let kind = match r.u()? {
                        0 => PivotKind::Method,
                        1 => PivotKind::Getter,
                        2 => PivotKind::Setter,
                        3 => PivotKind::Field { final_: false },
                        4 => PivotKind::Field { final_: true },
                        _ => return None,
                    };
                    Some(KeyPivot {
                        class,
                        member,
                        kind,
                        pivot: unopt(r.u()?)?,
                        library: r.w()?,
                        needs_member: r.u()? != 0,
                    })
                }
            };
            IdentFact::Target(IndexTarget {
                kind,
                concrete,
                name,
                word,
                group,
                declarations,
                declaration,
                answer,
                pivot,
            })
        }
        _ => return None,
    })
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
        let extension_type = r.u()? != 0;
        let mixins = r.list(|r| r.w())?;
        let raw_members =
            r.list(|r| Some((r.w()?, r.u()? != 0, r.w()?, u8::try_from(r.u()?).ok()?)))?;
        Some(ClassFact {
            key,
            library,
            extension,
            supers,
            written,
            members,
            fields,
            constructor_parameters,
            extension_type,
            mixins,
            raw_members,
        })
    })?;
    let function_variables = r.list(|r| Some((r.w()?, r.w()?)))?;
    let accessors = r.list(|r| Some((r.w()?, unopt(r.u()?)?, unopt(r.u()?)?)))?;
    let ambiguous = r.list(|r| r.w())?;
    let imports = r.list(|r| r.path())?;
    let mut last = 0u32;
    let idents = r.list(|r| {
        last = last.checked_add(r.w()?)?;
        Some((last, r.w()?))
    })?;
    let facts = r.list(read_fact)?;
    let mut last = 0u32;
    let prepares = r.list(|r| {
        last = last.checked_add(r.w()?)?;
        Some((last, r.w()?))
    })?;
    let prepare_facts = r.list(|r| {
        Some(match r.u()? {
            0 => PrepareFact::Nothing,
            1 => {
                let start = r.w()?;
                let end = start.checked_add(r.w()?)?;
                PrepareFact::Range {
                    start,
                    end,
                    old: r.s()?.to_string(),
                }
            }
            2 => PrepareFact::Refused(r.s()?.to_string()),
            _ => return None,
        })
    })?;
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
        idents,
        facts,
        prepares,
        prepare_facts,
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
        for strings in [&self.keys, &self.words, &self.paths] {
            w.n(strings.len());
            for t in strings.texts() {
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
        for i in 0..3 {
            let n = r.n()?;
            let strings = match i {
                0 => &mut index.keys,
                1 => &mut index.words,
                _ => &mut index.paths,
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
        let keys: Vec<u32> = other.keys.texts().map(|t| self.keys.id(t)).collect();
        let words: Vec<u32> = other.words.texts().map(|t| self.words.id(t)).collect();
        let paths: Vec<u32> = other.paths.texts().map(|t| self.paths.id(t)).collect();
        let pa = |v: &mut u32| {
            if *v != NONE {
                *v = paths[*v as usize];
            }
        };
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
                c.mixins.iter_mut().for_each(k);
                for m in &mut c.raw_members {
                    w(&mut m.0);
                    k(&mut m.2);
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
            for f in &mut r.facts {
                if let IdentFact::Local {
                    positional: Some(p),
                    ..
                } = f
                {
                    k(p);
                }
                let IdentFact::Target(t) = f else { continue };
                match &mut t.kind {
                    TargetKey::Top(x) | TargetKey::Constructor(x) => k(x),
                    TargetKey::Member { owner, name, .. } => {
                        k(owner);
                        w(name);
                    }
                }
                match &mut t.concrete {
                    Some(ConcreteKey::Function(x) | ConcreteKey::Variable(x)) => k(x),
                    None => {}
                }
                w(&mut t.name);
                if let Some(x) = &mut t.word {
                    w(x);
                }
                t.group.iter_mut().for_each(k);
                for x in &mut t.declarations {
                    pa(&mut x.0);
                    k(&mut x.3);
                }
                if let Some(x) = &mut t.declaration {
                    pa(&mut x.0);
                    k(&mut x.1);
                }
                if let Some(x) = &mut t.answer {
                    pa(&mut x.0);
                }
                if let Some(v) = &mut t.pivot {
                    k(&mut v.class);
                    if let Some((n, _)) = &mut v.member {
                        w(n);
                    }
                    k(&mut v.pivot);
                    k(&mut v.library);
                }
            }
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
    /// Gives the targets of `part`'s records (made over the resident's
    /// program, whose SDK has the DDC patches) the SDK locations this index
    /// knows: a target declared in the SDK takes the declaration spans of the
    /// same target in this index's record of the file (the server's view of
    /// the SDK, without patches); one this index's record does not have loses
    /// its fact, and the search identifies it as before. `sdk` is the SDK's
    /// `lib/` (canonical key).
    pub(crate) fn adopt_sdk_locations(&self, part: &mut ReferenceIndex, sdk: &Path) {
        let sdk = sdk.to_string_lossy().into_owned();
        let in_sdk = |paths: &Strings, p: u32| p == NONE || paths.text(p).starts_with(&sdk);
        let ReferenceIndex {
            keys,
            words,
            paths,
            files,
            ..
        } = part;
        for r in files.iter_mut().flatten() {
            let old: HashMap<String, &IndexTarget> = self
                .file(&r.path)
                .map(|o| {
                    o.facts
                        .iter()
                        .filter_map(|f| match f {
                            IdentFact::Target(t) => {
                                Some((target_signature(t, &self.keys, &self.words), t))
                            }
                            IdentFact::Local { .. } => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut lost: HashSet<u32> = HashSet::new();
            for (i, f) in r.facts.iter_mut().enumerate() {
                let IdentFact::Target(t) = f else { continue };
                let sdk_located = t.answer.is_some_and(|(p, _, _)| in_sdk(paths, p))
                    || t.declarations.iter().any(|(p, ..)| in_sdk(paths, *p))
                    || t.declaration.is_some_and(|(p, _)| in_sdk(paths, p));
                if !sdk_located {
                    continue;
                }
                let Some(o) = old.get(&target_signature(t, keys, words)) else {
                    lost.insert(i as u32);
                    continue;
                };
                let path = |p: u32, paths: &mut Strings| {
                    if p == NONE {
                        NONE
                    } else {
                        paths.id(self.paths.text(p))
                    }
                };
                t.answer = o.answer.map(|(p, s, e)| (path(p, paths), s, e));
                t.declarations = o
                    .declarations
                    .iter()
                    .map(|&(p, s, e, l)| (path(p, paths), s, e, keys.id(self.keys.text(l))))
                    .collect();
                t.declaration = o
                    .declaration
                    .map(|(p, l)| (path(p, paths), keys.id(self.keys.text(l))));
            }
            if !lost.is_empty() {
                r.idents.retain(|(_, f)| !lost.contains(f));
            }
        }
    }

    /// The relations and declarations of `path`'s record as text (the
    /// comparison of two records' differences).
    pub(crate) fn describe(&self, path: &Path) -> std::collections::BTreeSet<String> {
        let Some(r) = self.file(path) else {
            return Default::default();
        };
        let mut v: std::collections::BTreeSet<String> = r
            .relations
            .iter()
            .map(|x| {
                format!(
                    "{}@{}-{} {:?}",
                    self.keys.text(x.key),
                    x.start,
                    x.end,
                    x.tag
                )
            })
            .collect();
        v.extend(
            r.declarations
                .iter()
                .map(|(k, s, e)| format!("decl {}@{s}-{e}", self.keys.text(*k))),
        );
        v.extend(
            r.referenced
                .iter()
                .map(|w| format!("ref {}", self.words.text(*w))),
        );
        v
    }

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
        if orphaned() {
            std::process::exit(3);
        }
        project.query.infer_libraries(&[lib]);
        let mut tables = Tables {
            keys: &mut index.keys,
            words: &mut index.words,
            paths: &mut index.paths,
        };
        let records = Builder.library(&project, lib, &root, &mut tables);
        for u in units {
            project.query.bodies.units[u.0 as usize] = Default::default();
            project.forget_unit(u);
        }
        index.put(records);
    }
    index.note_absent(files.iter().cloned());
    Some(index)
}

/// The encoded index ([`ReferenceIndex::encode`]) of the records of the
/// libraries of `files` (project files, canonical keys) made over the
/// resident's last analysis (`lent`), whose bodies it inferred: the edited
/// document's record without inferring its library again (docs/LSP.md,
/// "Décima sexta rodada"). Gives `lent` back; `None` when no library of the
/// analysis has one of `files`. `patch_keys`: the server's SDK view has the
/// patches the analysis's has ([`analyzer_view_has_patches`]); otherwise a
/// patched member keys as its declaration.
pub(crate) fn records_of_analysis(
    lent: dartforge_parity::analysis::LentAnalysis,
    files: &[PathBuf],
    cache: &mut ResidentCache,
    patch_keys: bool,
) -> (dartforge_parity::analysis::LentAnalysis, Option<Vec<u8>>) {
    let dartforge_parity::analysis::LentAnalysis {
        program,
        interner,
        table,
        core,
        outline,
        bodies,
        libraries,
        generation,
        touched,
    } = lent;
    // What the earlier loans computed of the units that did not change since.
    let mut changed: HashSet<UnitId> = HashSet::new();
    if cache.generation == generation && cache.seen <= touched.len() {
        cache.reused += 1;
        changed.extend(touched[cache.seen..].iter().copied());
        for l in &libraries {
            changed.extend(program.library(*l).units.iter().copied());
        }
        for u in &changed {
            cache.referenced.remove(u);
        }
    } else {
        cache.referenced.clear();
        cache.subtypes = None;
    }
    cache.generation = generation;
    cache.seen = touched.len();
    let wanted: HashSet<&Path> = files.iter().map(PathBuf::as_path).collect();
    let of_files: Vec<LibraryId> = libraries
        .iter()
        .copied()
        .filter(|l| {
            let lib = program.library(*l);
            !lib.is_sdk
                && lib.units.iter().any(|u| {
                    program.unit(*u).path.as_deref().is_some_and(|c| {
                        wanted.contains(dartforge_elements::generated::key_of(c).as_ref())
                    })
                })
        })
        .collect();
    let root = files
        .first()
        .map(|f| crate::project::project_root(f))
        .unwrap_or_default();
    let project = Project {
        query: crate::query::Query {
            program,
            names: interner,
            table,
            core,
            outline,
            bodies,
            scope: None,
            refused: None,
        },
        root: root.clone(),
        libraries: of_files.iter().copied().collect(),
        referenced_names: std::sync::Mutex::new(std::mem::take(&mut cache.referenced)),
        reverse_index: Default::default(),
        comments: Default::default(),
        occurrences_by_unit: Default::default(),
        written_subtypes: Default::default(),
        patch_keys,
    };
    if let Some(old) = cache.subtypes.take() {
        let old = std::sync::Arc::try_unwrap(old).unwrap_or_else(|a| (*a).clone());
        let now = project.written_subtypes_after(old, &changed);
        if let Ok(mut m) = project.written_subtypes.lock() {
            *m = Some(std::sync::Arc::new(now));
        }
    }
    let mut index = ReferenceIndex::new(root.clone());
    for &lib in &of_files {
        let mut tables = Tables {
            keys: &mut index.keys,
            words: &mut index.words,
            paths: &mut index.paths,
        };
        let records = Builder.library(&project, lib, &root, &mut tables);
        index.put(records.into_iter().filter(|r| !r.frozen).collect());
    }
    let encoded = (!of_files.is_empty()).then(|| index.encode());
    cache.referenced = project.referenced_names.into_inner().unwrap_or_default();
    cache.subtypes = project.written_subtypes.into_inner().ok().flatten();
    let crate::query::Query {
        program,
        names,
        table,
        core,
        outline,
        ..
    } = project.query;
    let lent = dartforge_parity::analysis::LentAnalysis {
        program,
        interner: names,
        table,
        core,
        outline,
        bodies: Default::default(),
        libraries,
        generation,
        touched,
    };
    (lent, encoded)
}

/// What [`records_of_analysis`] keeps between loans of the resident's
/// analysis: the referenced names of the units that did not change (the
/// implicit `Object` supertypes of the member families read them from every
/// unit that cites `Object`, and computing them again took most of a record).
#[derive(Default)]
pub(crate) struct ResidentCache {
    generation: u64,
    seen: usize,
    referenced: HashMap<UnitId, std::sync::Arc<HashSet<String>>>,
    /// [`Project::written_subtypes`] of the last loan.
    subtypes: Option<std::sync::Arc<crate::project::WrittenSubtypes>>,
    /// Loans that reused what an earlier one computed (tests and measurement).
    pub(crate) reused: usize,
}

/// Whether the SDK the server sees (`SdkLayout::load_as_analyzer`) applies
/// patches: the analyzer's view has none, unless the SDK lacks the analyzer's
/// library list and the DDC profile stands in for it.
pub(crate) fn analyzer_view_has_patches(sdk_lib: &Path) -> bool {
    dartforge_elements::sdk::SdkLayout::load_as_analyzer(sdk_lib)
        .is_ok_and(|s| s.libraries.values().any(|l| !l.patches.is_empty()))
}

/// The text of what an [`IndexTarget`] searches (everything but its
/// locations), to find the same target in another index's record.
fn target_signature(t: &IndexTarget, keys: &Strings, words: &Strings) -> String {
    let k = |v: u32| if v == NONE { "" } else { keys.text(v) };
    let kind = match &t.kind {
        TargetKey::Top(x) => format!("T{}", k(*x)),
        TargetKey::Member {
            owner,
            name,
            is_static,
        } => format!("M{} {} {is_static}", k(*owner), words.text(*name)),
        TargetKey::Constructor(x) => format!("K{}", k(*x)),
    };
    let concrete = match t.concrete {
        Some(ConcreteKey::Function(x)) => format!("f{}", k(x)),
        Some(ConcreteKey::Variable(x)) => format!("v{}", k(x)),
        None => String::new(),
    };
    let group: Vec<&str> = t.group.iter().map(|g| k(*g)).collect();
    format!(
        "{kind}|{concrete}|{}|{:?}|{group:?}",
        words.text(t.name),
        t.word.map(|w| words.text(w))
    )
}
