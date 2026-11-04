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
//! `.dart_tool/build/generated`): the analyzer's `PackageBuildWorkspace`
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
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// The handler's result limit.
pub(crate) const LIMIT: usize = 500;

// -- ElementNameUnion ---------------------------------------------------------

/// `ElementNameUnion`: for each letter position (up to 63), the letters that
/// some name of the library has there; `mask[0]` is the longest length.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
struct Declaration {
    name: String,
    kind: Kind,
    /// `className ?? mixinName`.
    container: Option<String>,
    signature: Signature,
    /// The code range in LSP positions of the file.
    start: (u32, u32),
    end: (u32, u32),
}

#[derive(Debug)]
struct Unit {
    uri: String,
    declarations: Vec<Declaration>,
}

#[derive(Debug)]
struct Library {
    union: NameUnion,
    units: Vec<Unit>,
    types: LocalTypes,
    /// The URIs its units import, with the unit's file.
    imports: Vec<(PathBuf, String)>,
    /// Exports are discovered before imports by FileState.
    exports: Vec<(PathBuf, String)>,
}

#[derive(Debug)]
struct SdkFiles {
    root: PathBuf,
    layout: dartforge_elements::sdk::SdkLayout,
    bootstrap: Vec<PathBuf>,
    available: Vec<PathBuf>,
}

/// The fingerprint of a read file: its size and date on disk, or the open
/// text (by size and a digest).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fingerprint {
    Disk(Option<SystemTime>, u64),
    Open(usize, u64),
}

#[derive(Debug)]
struct Entry {
    /// The library's file and its parts, with their fingerprints.
    files: Vec<(PathBuf, Fingerprint)>,
    library: Option<Arc<Library>>,
}

/// The index: the libraries already read, by path.
#[derive(Debug, Default)]
pub(crate) struct Index {
    entries: HashMap<PathBuf, Entry>,
    /// The SDK's bootstrap and discovery order, by `lib/`.
    sdk: Option<Arc<SdkFiles>>,
    /// The `.dart` files of each package's `lib/`, as last listed.
    listed: HashMap<PathBuf, Vec<PathBuf>>,
    /// The last search's files, with what they were listed from.
    #[allow(clippy::type_complexity)]
    paths: Option<(
        (
            Vec<PathBuf>,
            Option<PathBuf>,
            Vec<(String, PathBuf)>,
            Vec<(String, PathBuf, PathBuf)>,
        ),
        Vec<PathBuf>,
    )>,
}

/// Reads a file (open or from disk) with its fingerprint.
type Reader<'a> = dyn Fn(&Path) -> Option<(String, Fingerprint)> + 'a;

/// The fingerprint of a file on disk, without reading it.
fn disk_fingerprint(path: &Path) -> Option<Fingerprint> {
    let m = std::fs::metadata(path).ok()?;
    Some(Fingerprint::Disk(m.modified().ok(), m.len()))
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
        container: Option<String>,
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
        container: Option<&str>,
        constructors: bool,
        first_fields: Vec<Declaration>,
        first_constructors: Vec<Declaration>,
    ) {
        let l = self.l;
        let ast = l.ast;
        let container = container.map(str::to_string);
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
                    self.members(&c.members, Some(&name), true, Vec::new(), Vec::new());
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
                    self.members(&m.members, Some(&name), true, Vec::new(), Vec::new());
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
                Some(name.clone()),
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
                Some(name.clone()),
                Signature::List { empty: false },
                ctor_start,
                x.representation_span.end,
            );
            self.members(&x.members, Some(&name), true, vec![field], vec![ctor]);
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
            if first {
                return (vec![(file, Fingerprint::Disk(None, 0))], None);
            }
            continue;
        };
        files.push((file.clone(), fingerprint));
        let mut names = Interner::new();
        let analyzed = dartforge_frontend::parser::parse(&source, &mut names);
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
        for d in &analyzed.unit.directives {
            let (uri, output) = match &d.kind {
                DirectiveKind::Import { uri, .. } => (uri, &mut imports),
                DirectiveKind::Export { uri, .. } => (uri, &mut exports),
                _ => continue,
            };
            if let Some(u) = dartforge_elements::load::string_lit_value(uri) {
                output.push((file.clone(), u.to_string()));
            }
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
    fn library(
        &mut self,
        path: &Path,
        read: &Reader<'_>,
        open: &HashMap<PathBuf, Fingerprint>,
        uri_of: &dyn Fn(&Path) -> String,
        trust_disk: bool,
    ) -> Option<Arc<Library>> {
        // With the disk trusted (the watcher reported nothing since a recent
        // comparison), a file that is not open is as it was indexed.
        let current =
            |p: &Path, i: &Fingerprint| match open.get(&dartforge_elements::generated::key(p)) {
                Some(o) => Some(o.clone()),
                None if trust_disk && matches!(i, Fingerprint::Disk(..)) => Some(i.clone()),
                None => disk_fingerprint(p),
            };
        if let Some(e) = self.entries.get(path)
            && e.files
                .iter()
                .all(|(p, i)| current(p, i).as_ref() == Some(i))
        {
            return e.library.clone();
        }
        let (files, library) = index(path, read, uri_of);
        let library = library.map(Arc::new);
        self.entries.insert(
            path.to_path_buf(),
            Entry {
                files,
                library: library.clone(),
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
                        let uri = match &directive.kind {
                            DirectiveKind::Export { uri, .. } if exports => uri,
                            DirectiveKind::Import { uri, .. } if !exports => uri,
                            _ => continue,
                        };
                        if let Some(uri) = dartforge_elements::load::string_lit_value(uri)
                            && let Some(name) = uri.strip_prefix("dart:")
                            && let Some(library) = layout.library(name)
                        {
                            list.push(dartforge_elements::generated::key(&library.path));
                        }
                    }
                }
            }
        }
        let files = Arc::new(SdkFiles {
            root: lib.to_path_buf(),
            layout,
            bootstrap: list,
            available: read_libraries_dart(lib),
        });
        self.sdk = Some(files.clone());
        files
    }

    /// `FindDeclarations.compute` + the handler's `_asSymbolInformation`.
    /// `added` are the project files; `packages`, the packages' `lib/`s (in
    /// `package_config.json` order); `open_texts`, the URI and text of the open
    /// documents by normalized path (`gerado::key`); `kinds`, the
    /// `SymbolKind`s the client accepts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search(
        &mut self,
        query: &str,
        added: &[PathBuf],
        sdk: Option<&Path>,
        packages: &[(String, PathBuf)],
        generated: &[(String, PathBuf, PathBuf)],
        open_texts: &HashMap<PathBuf, (String, String)>,
        kinds: Option<&[u64]>,
        trust_disk: bool,
    ) -> Vec<Value> {
        // Open documents count through the editor's text (the overlay), with the
        // client's URI.
        let open: HashMap<PathBuf, Fingerprint> = open_texts
            .iter()
            .map(|(k, (_, t))| (k.clone(), text_fingerprint(t)))
            .collect();
        let read = |p: &Path| -> Option<(String, Fingerprint)> {
            let k = dartforge_elements::generated::key(p);
            if let (Some((_, t)), Some(i)) = (open_texts.get(&k), open.get(&k)) {
                return Some((t.clone(), i.clone()));
            }
            let i = disk_fingerprint(p)?;
            let t = std::fs::read_to_string(p).ok()?;
            Some((t, i))
        };
        let read: &Reader<'_> = &read;
        let open = &open;
        let uri_of = |p: &Path| -> String {
            match open_texts.get(&dartforge_elements::generated::key(p)) {
                Some((uri, _)) => uri.clone(),
                None => url::Url::from_file_path(p)
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
            }
        };
        // The files in `OwnedFiles` order (added ones, then the known ones that
        // were not added), kept while the disk is trusted and the inputs are the
        // same.
        let inputs = (
            added.to_vec(),
            sdk.map(Path::to_path_buf),
            packages.to_vec(),
            generated.to_vec(),
        );
        if trust_disk
            && let Some((last, paths)) = &self.paths
            && *last == inputs
        {
            let paths = paths.clone();
            return self.find(query, &paths, read, open, &uri_of, kinds, trust_disk);
        }
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        for p in added {
            include_path(p.clone(), &mut paths, &mut seen);
        }
        let sdk_files = sdk.map(|lib| self.sdk_files(lib));
        if let Some(sdk) = &sdk_files {
            for p in &sdk.bootstrap {
                include_path(p.clone(), &mut paths, &mut seen);
            }
        }
        // Analysis discovers imported libraries before discoverAvailableFiles
        // scans the remaining SDK and package files. Create each library's
        // direct references before visiting them, as FileState's lazy lists do.
        let mut pending: Vec<PathBuf> = added.iter().rev().cloned().collect();
        let mut visited = HashSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(dartforge_elements::generated::key(&path)) {
                continue;
            }
            let Some(library) = self.library(&path, read, open, &uri_of, trust_disk) else {
                continue;
            };
            let mut next = Vec::new();
            for (unit, uri) in library.exports.iter().chain(&library.imports) {
                if let Some(target) =
                    dependency_target(unit, uri, sdk_files.as_deref(), packages, generated)
                {
                    include_path(target.clone(), &mut paths, &mut seen);
                    next.push(target);
                }
            }
            if let Some(core) = sdk_files.as_ref().and_then(|s| s.layout.library("core")) {
                next.push(core.path.clone());
            }
            pending.extend(next.into_iter().rev());
        }
        if let Some(sdk) = &sdk_files {
            for p in &sdk.available {
                include_path(p.clone(), &mut paths, &mut seen);
            }
        }
        for (_, folder) in packages {
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
                include_path(p, &mut paths, &mut seen);
            }
        }
        self.paths = Some((inputs, paths.clone()));
        self.find(query, &paths, read, open, &uri_of, kinds, trust_disk)
    }

    /// The declarations of the libraries at `paths` that match `query`.
    #[allow(clippy::too_many_arguments)]
    fn find(
        &mut self,
        query: &str,
        paths: &[PathBuf],
        read: &Reader<'_>,
        open: &HashMap<PathBuf, Fingerprint>,
        uri_of: &dyn Fn(&Path) -> String,
        kinds: Option<&[u64]>,
        trust_disk: bool,
    ) -> Vec<Value> {
        let libraries: Vec<Arc<Library>> = paths
            .iter()
            .filter_map(|c| self.library(c, read, open, uri_of, trust_disk))
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

/// Adds a known file once, preserving the discovery order.
fn include_path(path: PathBuf, paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) -> bool {
    let new = seen.insert(dartforge_elements::generated::key(&path));
    if new {
        paths.push(path);
    }
    new
}

/// Resolves a written dependency through the analyzer SDK and package map,
/// including the generated package workspace used by build_runner.
fn dependency_target(
    unit: &Path,
    uri: &str,
    sdk: Option<&SdkFiles>,
    packages: &[(String, PathBuf)],
    generated: &[(String, PathBuf, PathBuf)],
) -> Option<PathBuf> {
    if let Some(name) = uri.strip_prefix("dart:") {
        return sdk?.layout.library(name).map(|l| l.path.clone());
    }
    if let Some(rest) = uri.strip_prefix("package:") {
        let (name, relative) = rest.split_once('/')?;
        let (_, lib) = packages.iter().find(|(n, _)| n == name)?;
        return generated_target(unit, uri, generated).or_else(|| Some(lib.join(relative)));
    }
    generated_target(unit, uri, generated).or_else(|| {
        url::Url::from_file_path(unit)
            .ok()?
            .join(uri)
            .ok()?
            .to_file_path()
            .ok()
    })
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
fn read_libraries_dart(lib: &Path) -> Vec<PathBuf> {
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
        rest = &rest[i + "LibraryInfo(".len()..];
        let entry_text = rest.trim_start();
        let Some(quotes) = entry_text
            .chars()
            .next()
            .filter(|c| *c == '\'' || *c == '"')
        else {
            continue;
        };
        let content = &entry_text[1..];
        let Some(f) = content.find(quotes) else {
            continue;
        };
        let relative = &content[..f];
        output.push(dartforge_elements::generated::key(&lib.join(relative)));
    }
    output
}

/// The names and `lib/` directories (the `packageUri`) of the packages of
/// `root`'s `.dart_tool/package_config.json`, in file order.
pub(crate) fn package_folders(root: &Path) -> Vec<(String, PathBuf)> {
    crate::project::packages_with_name(root)
}

/// The generated file `uri` (imported or exported by `unit`) points to, when
/// it is a `build_runner` output: a `package:` URI of a package in
/// `generated` (name, `lib/` folder, generated `lib/` folder) whose file is
/// not in the package's `lib/` but is under the generated one, or a relative
/// URI from a generated file to another one.
fn generated_target(
    unit: &Path,
    uri: &str,
    generated: &[(String, PathBuf, PathBuf)],
) -> Option<PathBuf> {
    if let Some(rest) = uri.strip_prefix("package:") {
        let (name, path) = rest.split_once('/')?;
        let (_, lib, out) = generated.iter().find(|(n, ..)| n == name)?;
        if lib.join(path).is_file() {
            return None;
        }
        let g = out.join(path);
        return g.is_file().then_some(g);
    }
    if uri.contains(':') {
        return None;
    }
    // A relative URI: from a generated file, or from a package file to an
    // output next to it.
    let folder = unit.parent()?;
    let target = dartforge_elements::generated::key(&folder.join(uri));
    if target.is_file() {
        return generated
            .iter()
            .any(|(_, _, out)| target.starts_with(out))
            .then_some(target);
    }
    let (_, lib, out) = generated
        .iter()
        .find(|(_, lib, _)| folder.starts_with(lib))?;
    let g = out.join(target.strip_prefix(lib).ok()?);
    g.is_file().then_some(g)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                Some(&root),
                &[],
                &[],
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
            "export 'detail.dart'; class NeedleApi {}\n",
        )
        .unwrap();
        std::fs::write(package.join("detail.dart"), "class NeedleDetail {}\n").unwrap();
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
            Some(&root),
            &[("dependency".to_string(), package)],
            &[],
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
