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
}

impl Tag {
    fn code(self) -> u32 {
        self as u32
    }

    fn from_code(c: u32) -> Option<Self> {
        Some(match c {
            0 => Tag::Element,
            1 => Tag::TypeName,
            2 => Tag::Function,
            3 => Tag::Variable,
            4 => Tag::ConstructorClassName,
            5 => Tag::Combinator,
            _ => return None,
        })
    }
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
    /// Something no top-level or member search matches by target (a local, a
    /// constructor, a prefix); it can still match a member by its concrete
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
    /// The names of the `this.x` parameters and of the `x = e` initializers of
    /// its constructors.
    pub field_parameters: Vec<u32>,
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
            FunctionKind::ImplicitAccessor => 'i',
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
pub(crate) struct Builder {
    /// The elements declared in each unit.
    by_unit: Vec<Vec<Declared>>,
}

/// The program and the keys of one [`Builder::library`] call.
struct Emit<'a> {
    project: &'a Project,
    keys: Keys<'a>,
}

impl Builder {
    pub(crate) fn new(project: &Project) -> Self {
        let p = project.program();
        let mut by_unit: Vec<Vec<Declared>> = vec![Vec::new(); p.units.len()];
        let first_unit = |lib: LibraryId| p.library(lib).units.first().copied();
        for (i, c) in p.classes.iter().enumerate() {
            if let Some(u) = c.decl.map(|d| d.unit).or_else(|| first_unit(c.library)) {
                by_unit[u.0 as usize].push(Declared::Class(ClassId(i as u32)));
            }
        }
        for (i, x) in p.extensions.iter().enumerate() {
            by_unit[x.decl.unit.0 as usize].push(Declared::Extension(ExtensionId(i as u32)));
        }
        for (i, t) in p.typedefs.iter().enumerate() {
            by_unit[t.decl.unit.0 as usize].push(Declared::Typedef(TypedefId(i as u32)));
        }
        for (i, f) in p.functions.iter().enumerate() {
            let unit = match f.node {
                FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => {
                    Some(unit)
                }
                FunctionRef::None => f
                    .variable
                    .and_then(|v| p.variable(v).node.unit())
                    .or_else(|| f.class.and_then(|c| p.class(c).decl.map(|d| d.unit)))
                    .or_else(|| first_unit(f.library)),
            };
            if let Some(u) = unit {
                by_unit[u.0 as usize].push(Declared::Function(FunctionElementId(i as u32)));
            }
        }
        for (i, v) in p.variables.iter().enumerate() {
            if let Some(u) = v.node.unit().or_else(|| first_unit(v.library)) {
                by_unit[u.0 as usize].push(Declared::Variable(VariableId(i as u32)));
            }
        }
        Builder { by_unit }
    }

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
        emit.library(&self.by_unit, lib, root, keys, words)
    }
}

impl Emit<'_> {
    fn library(
        &mut self,
        by_unit: &[Vec<Declared>],
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
            records.push((
                u,
                FileRecord {
                    frozen: !inside(&path, root),
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
            let declared = by_unit.get(u.0 as usize).cloned().unwrap_or_default();
            for d in declared {
                let (key, name) = match d {
                    Declared::Class(c) => {
                        let key = self.keys.class(c);
                        let fact = self.class_fact(c, keys, words);
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
                        target,
                        concrete: concrete_key,
                        field_parameter,
                    });
                }
            }
        }
    }

    fn class_fact(&mut self, c: ClassId, keys: &mut Strings, words: &mut Strings) -> ClassFact {
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
        let mut field_parameters = Vec::new();
        for (u, m) in p.class_members(c) {
            let MemberKind::Constructor(k) = &p.unit(u).ast.member(m).kind else {
                continue;
            };
            for q in k.parameters.iter() {
                if q.this_
                    && let Some(n) = q.name
                {
                    field_parameters.push(words.id(project.name(n.sym)));
                }
            }
            for i in k.initializers.iter() {
                if let ast::Initializer::Field { name, .. } = i {
                    field_parameters.push(words.id(project.name(name.sym)));
                }
            }
        }
        field_parameters.sort_unstable();
        field_parameters.dedup();
        ClassFact {
            key: keys.id(&self.keys.class(c)),
            library: keys.id(&p.library(cl.library).uri),
            extension: false,
            supers,
            written,
            members,
            fields,
            field_parameters,
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
            },
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
                if !*is_static {
                    let w = self.words.get(member);
                    let has_parameters = family.classes.iter().chain([&owner]).any(|c| {
                        self.class_fact(d, *c)
                            .is_some_and(|f| w.is_some_and(|w| f.field_parameters.contains(&w)))
                    });
                    if has_parameters {
                        return Err(Miss::Unsupported("a field with this.x parameters"));
                    }
                }
                Covers::Member {
                    family,
                    owner,
                    is_static: *is_static,
                }
            }
            Target::Constructor(_) => return Err(Miss::Unsupported("a constructor")),
            _ => return Err(Miss::Unsupported("a local search")),
        };
        // Ambiguous keys.
        let involved: Vec<u32> = match &covers {
            Covers::Top(set) => set.iter().copied().collect(),
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
            Covers::Top(_) => {
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
                Covers::Member { family, .. } => {
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
                uses.push((f, doc.start, doc.end));
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
    let builder = Builder::new(&project);
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
