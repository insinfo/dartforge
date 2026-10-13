//! `workspace/symbol` como o servidor do Dart 3.6.2 (docs/LSP-ESPECIFICACAO.md
//! §5): o `WorkspaceSymbolHandler` sobre o `FindDeclarations` do analyzer.
//!
//! * as bibliotecas vêm na ordem do `OwnedFiles`: primeiro os arquivos
//!   adicionados (os do projeto), depois os conhecidos pelo
//!   `discoverAvailableFiles` (as bibliotecas do SDK na ordem do
//!   `libraries.dart` e os `lib/` dos pacotes do `package_config.json`); uma
//!   parte não é biblioteca e fica de fora (o conteúdo dela entra pela
//!   biblioteca dona);
//! * cada biblioteca passa pelo pré-filtro `ElementNameUnion.contains`;
//! * por unidade, na ordem do `_FindCompilationUnitDeclarations`: acessores
//!   de topo, classes (com os acessores, construtores, campos e métodos),
//!   enums, mixins, extensões, extension types, funções, typedefs e
//!   variáveis; só os elementos não sintéticos;
//! * o nome casa pelo `FuzzyMatcher` (`MatchStyle.TEXT`) com score ≥ 0, sem
//!   ordenar; a coleta para em 500;
//! * o item: o nome com `()`/`(…)` pelo `getDisplayString()` do executável,
//!   a espécie pelo `declarationKindToSymbolKind`, o code range do elemento
//!   e o `containerName` de classe ou mixin.
//!
//! O índice é só sintático (o parser, sem resolução) e fica em cache por
//! arquivo; o de uma biblioteca é refeito quando ela ou uma das partes muda.

use crate::fuzzy_matcher::{Matcher, Style};
use dartforge_frontend::ast::{self, Annotation, Ast, DeclKind, DirectiveKind, FunctionKind, MemberKind, TypeKind};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_intern::Interner;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// O limite de resultados do handler.
pub(crate) const LIMIT: usize = 500;

// -- ElementNameUnion ---------------------------------------------------------

/// `ElementNameUnion`: para cada posição de letra (até 63), as letras que
/// algum nome da biblioteca tem nela; `mask[0]` é o maior comprimento.
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

// -- Resumo de tipos (para o `getDisplayString()`) -----------------------------

/// O que importa de um tipo escrito para saber onde fica o primeiro `(` do
/// `getDisplayString()` do executável.
#[derive(Debug, Clone)]
enum Summary {
    /// `Name<args>` (o último segmento de `p.Name`).
    Name { name: String, args: Vec<Summary> },
    /// `R Function(…)`, com o retorno escrito.
    Function(Option<Box<Summary>>),
    /// `(int, String)`.
    Registration,
    Void,
    /// O parâmetro de tipo `i` do `typedef` que está sendo expandido.
    Parameter(usize),
}

/// Um `typedef`: o tipo apelidado (os parâmetros de tipo viram
/// [`Summary::Parameter`]).
#[derive(Debug, Clone)]
struct Alias {
    target: Summary,
}

/// Onde o `(` aparece no texto exibido de um tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paren {
    None,
    Contains,
    /// O texto começa com `(` (um registro).
    StartsWith,
}

/// Os tipos declarados numa biblioteca: `Some` num `typedef`, `None` numa
/// classe, mixin, enum ou extension type (que esconde um `typedef` homônimo
/// de outra biblioteca).
type LocalTypes = HashMap<String, Option<Alias>>;

fn paren(r: &Summary, ctx: &[Paren], locals: &LocalTypes, global: &HashMap<String, Alias>, depth: u32) -> Paren {
    match r {
        Summary::Void => Paren::None,
        Summary::Registration => Paren::StartsWith,
        Summary::Parameter(i) => ctx.get(*i).copied().unwrap_or(Paren::None),
        // `R Function(…)`: começa com `(` só quando o retorno começa.
        Summary::Function(return_) => match return_ {
            Some(x) if paren(x, ctx, locals, global, depth) == Paren::StartsWith => Paren::StartsWith,
            _ => Paren::Contains,
        },
        Summary::Name { name, args } => {
            let arguments: Vec<Paren> = args.iter().map(|a| paren(a, ctx, locals, global, depth)).collect();
            if depth < 16 {
                let alias = match locals.get(name) {
                    Some(local) => local.as_ref(),
                    None => global.get(name),
                };
                // O `getDisplayString()` sem `preferTypeAlias` mostra o tipo
                // apelidado.
                if let Some(a) = alias {
                    return paren(&a.target, &arguments, locals, global, depth + 1);
                }
            }
            if arguments.iter().any(|p| *p != Paren::None) { Paren::Contains } else { Paren::None }
        }
    }
}

fn summarize(ast: &Ast, names: &Interner, t: ast::TypeId, parameters: &[String]) -> Summary {
    let ty = ast.ty(t);
    match &ty.kind {
        TypeKind::Void => Summary::Void,
        TypeKind::Record { .. } => Summary::Registration,
        TypeKind::Function { return_type, .. } => Summary::Function(return_type.map(|r| Box::new(summarize(ast, names, r, parameters)))),
        TypeKind::Named { name, args } => {
            let name_ = name.last().map(|n| names.resolve(n.sym).to_string()).unwrap_or_default();
            if name.len() == 1
                && args.is_empty()
                && let Some(i) = parameters.iter().position(|p| *p == name_)
            {
                return Summary::Parameter(i);
            }
            Summary::Name { name: name_, args: args.iter().map(|a| summarize(ast, names, *a, parameters)).collect() }
        }
    }
}

// -- Declarações ----------------------------------------------------------------

/// `DeclarationKind` do analyzer (as usadas pelo `_getSearchElementKind`).
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
    /// As preferências do `declarationKindToSymbolKind`.
    fn preferences(self) -> &'static [u64] {
        match self {
            Kind::Class | Kind::ClassAlias | Kind::Extension | Kind::ExtensionType | Kind::Mixin | Kind::Typedef => &[5],
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

/// O que o `getDisplayString()` do elemento precisa para o sufixo.
#[derive(Debug, Clone)]
enum Signature {
    /// Não é executável.
    None,
    /// `A A.n(…)`: o primeiro `(` é o da lista.
    List { empty: bool },
    /// `R get x`: só o retorno pode ter `(`.
    Getter { return_: Option<Summary>, augment: bool },
    /// `R name<T>(…)`.
    Executable { return_: Option<Summary>, empty: bool, augment: bool },
}

#[derive(Debug, Clone)]
struct Declaration {
    name: String,
    kind: Kind,
    /// `className ?? mixinName`.
    container: Option<String>,
    signature: Signature,
    /// O code range em posições LSP do arquivo.
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
}

/// A impressão de um arquivo lido: o tamanho e a data no disco, ou o texto
/// aberto (pelo tamanho e um resumo).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fingerprint {
    Disk(Option<SystemTime>, u64),
    Open(usize, u64),
}

#[derive(Debug)]
struct Entry {
    /// O arquivo da biblioteca e as partes, com as impressões.
    files: Vec<(PathBuf, Fingerprint)>,
    library: Option<Arc<Library>>,
}

/// O índice: as bibliotecas já lidas, pelo caminho.
#[derive(Debug, Default)]
pub(crate) struct Index {
    entries: HashMap<PathBuf, Entry>,
    /// As bibliotecas do SDK na ordem do `libraries.dart`, pelo `lib/`.
    sdk: Option<(PathBuf, Vec<PathBuf>)>,
}

/// Lê um arquivo (aberto ou do disco) com a sua impressão.
type Reader<'a> = dyn Fn(&Path) -> Option<(String, Fingerprint)> + 'a;

/// A impressão de um arquivo no disco, sem lê-lo.
fn disk_fingerprint(path: &Path) -> Option<Fingerprint> {
    let m = std::fs::metadata(path).ok()?;
    Some(Fingerprint::Disk(m.modified().ok(), m.len()))
}

/// A impressão de um texto aberto.
fn text_fingerprint(text: &str) -> Fingerprint {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    Fingerprint::Open(text.len(), h.finish())
}

/// A primeira posição de código a partir de `pos` (pula espaços e
/// comentários).
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

/// O que lê uma unidade: o texto, as linhas e os comentários.
struct Read<'a> {
    source: &'a str,
    ast: &'a Ast,
    names: &'a Interner,
    lines: crate::utf16::LineTable,
    comments: Comentarios,
}

impl Read<'_> {
    fn name(&self, n: ast::Name) -> String {
        self.names.resolve(n.sym).to_string()
    }

    /// O `offset` de um `AnnotatedNode`: o comentário de documentação e as
    /// anotações (`AnnotatedNodeImpl.beginToken`).
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

    fn declaration(&self, name: String, kind: Kind, container: Option<String>, signature: Signature, start: usize, end: usize) -> Declaration {
        Declaration { name, kind, container, signature, start: self.position(start), end: self.position(end) }
    }

    /// O fim de uma variável: o do inicializador, senão o do nome.
    fn variable_end(&self, v: &ast::Variable) -> usize {
        v.initializer.map_or(v.name.span.end, |e| self.ast.expr(e).span.end)
    }
}

/// O conteúdo de uma unidade para a lista de declarações e a união.
struct Collector<'a, 'b> {
    l: &'b Read<'a>,
    output: Vec<Declaration>,
    union: &'b mut NameUnion,
    types: &'b mut LocalTypes,
}

impl Collector<'_, '_> {
    /// `_addDeclaration` sem o casamento (que é feito na consulta).
    fn push(&mut self, d: Declaration) {
        self.output.push(d);
    }

    fn function_signature(&self, f: &ast::Function, augment: bool, type_parameters: &[String]) -> Signature {
        let return_ = f.return_type.map(|t| summarize(self.l.ast, self.l.names, t, type_parameters));
        match f.kind {
            FunctionKind::Getter => Signature::Getter { return_, augment },
            FunctionKind::Setter => Signature::List { empty: f.parameters.as_ref().is_none_or(|p| p.is_empty()) },
            FunctionKind::Function | FunctionKind::Operator => {
                Signature::Executable { return_, empty: f.parameters.as_ref().is_none_or(|p| p.is_empty()), augment }
            }
        }
    }

    /// Os membros de uma classe, mixin, enum, extensão ou extension type, na
    /// ordem do `_addClasses`/`_addExtensions`: acessores, construtores,
    /// campos (`primeiros` antes dos declarados) e métodos.
    fn members(&mut self, members: &[ast::MemberId], container: Option<&str>, constructors: bool, first_fields: Vec<Declaration>, first_constructors: Vec<Declaration>) {
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
                        fields.push(l.declaration(name, Kind::Field, container.clone(), Signature::None, start_, l.variable_end(v)));
                    }
                }
                MemberKind::Method(fid) => {
                    let f = ast.function(*fid);
                    let Some(n) = f.name else { continue };
                    let name = l.name(n);
                    self.union.add(&name);
                    let signature = self.function_signature(f, m.augment, &[]);
                    match f.kind {
                        FunctionKind::Getter => accessors.push(l.declaration(name, Kind::Getter, container.clone(), signature, start, m.span.end)),
                        FunctionKind::Setter => accessors.push(l.declaration(name, Kind::Setter, container.clone(), signature, start, m.span.end)),
                        _ => methods.push(l.declaration(name, Kind::Method, container.clone(), signature, start, m.span.end)),
                    }
                }
                MemberKind::Constructor(c) => {
                    if !constructors {
                        continue;
                    }
                    let name = c.name.map(|n| l.name(n)).filter(|n| n != "new").unwrap_or_default();
                    ctors.push(l.declaration(name, Kind::Constructor, container.clone(), Signature::List { empty: c.parameters.is_empty() }, start, m.span.end));
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
            let DeclKind::Function(fid) = d.kind else { continue };
            let f = ast.function(fid);
            let Some(n) = f.name else { continue };
            let name = l.name(n);
            self.union.add(&name);
            let kind = if f.kind == FunctionKind::Getter { Kind::Getter } else { Kind::Setter };
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
                    let kind = if c.mixin_application { Kind::ClassAlias } else { Kind::Class };
                    self.push(l.declaration(name.clone(), kind, None, Signature::None, start, d.span.end));
                    self.members(&c.members, Some(&name), true, Vec::new(), Vec::new());
                }
                DeclKind::Enum(e) => {
                    let name = l.name(e.name);
                    self.union.add(&name);
                    self.types.insert(name.clone(), None);
                    self.push(l.declaration(name, Kind::Enum, None, Signature::None, start, d.span.end));
                    // As constantes são campos; o `values` sintético só
                    // entra na união.
                    let mut constants = Vec::new();
                    for k in &e.constants {
                        let n = l.name(k.name);
                        self.union.add(&n);
                        let ci = l.annotated_start(k.span.start, &k.metadata);
                        constants.push(l.declaration(n, Kind::EnumConstant, None, Signature::None, ci, k.span.end));
                    }
                    self.union.add("values");
                    self.members(&e.members, None, true, constants, Vec::new());
                }
                DeclKind::Mixin(m) => {
                    let name = l.name(m.name);
                    self.union.add(&name);
                    self.types.insert(name.clone(), None);
                    self.push(l.declaration(name.clone(), Kind::Mixin, None, Signature::None, start, d.span.end));
                    self.members(&m.members, Some(&name), true, Vec::new(), Vec::new());
                }
                _ => {}
            }
        }
        for did in extensions {
            let d = ast.decl(did);
            let DeclKind::Extension(x) = &d.kind else { continue };
            if let Some(n) = x.name {
                let name = l.name(n);
                self.union.add(&name);
                let start = l.annotated_start(d.span.start, &d.metadata);
                self.push(l.declaration(name, Kind::Extension, None, Signature::None, start, d.span.end));
            }
            self.members(&x.members, None, false, Vec::new(), Vec::new());
        }
        for did in extension_types {
            let d = ast.decl(did);
            let DeclKind::ExtensionType(x) = &d.kind else { continue };
            let name = l.name(x.name);
            self.union.add(&name);
            self.types.insert(name.clone(), None);
            let start = l.annotated_start(d.span.start, &d.metadata);
            self.push(l.declaration(name.clone(), Kind::ExtensionType, None, Signature::None, start, d.span.end));
            // O campo de representação e o construtor primário
            // (`_builtRepresentationDeclaration`) vêm antes dos declarados.
            let field_name_ = Some(l.name(x.representation_name)).filter(|n| !n.is_empty()).unwrap_or_else(|| "<empty>".to_string());
            self.union.add(&field_name_);
            let field_start = x.representation_metadata.first().map_or(l.ast.ty(x.representation_type).span.start, |m| m.span.start);
            let field = l.declaration(field_name_, Kind::Field, Some(name.clone()), Signature::None, field_start, x.representation_name.span.end);
            let (ctor_name, ctor_start) = match x.constructor {
                Some(n) => {
                    let text = l.name(n);
                    // O nó começa no `.` do nome.
                    let before = l.source[..n.span.start].trim_end();
                    let dot = if before.ends_with('.') { before.len() - 1 } else { n.span.start };
                    (if text == "new" { String::new() } else { text }, dot)
                }
                None => (String::new(), x.representation_span.start),
            };
            let ctor = l.declaration(ctor_name, Kind::Constructor, Some(name.clone()), Signature::List { empty: false }, ctor_start, x.representation_span.end);
            self.members(&x.members, Some(&name), true, vec![field], vec![ctor]);
        }
        for did in functions {
            let d = ast.decl(did);
            let DeclKind::Function(fid) = d.kind else { continue };
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
            let DeclKind::Typedef(t) = &d.kind else { continue };
            let name = l.name(t.name);
            self.union.add(&name);
            let parameters: Vec<String> = t.type_params.iter().map(|p| l.name(p.name)).collect();
            let target = match &t.kind {
                ast::TypedefKind::Alias(a) => summarize(ast, l.names, *a, &parameters),
                ast::TypedefKind::Legacy { return_type, .. } => Summary::Function(return_type.map(|r| Box::new(summarize(ast, l.names, r, &parameters)))),
            };
            self.types.insert(name.clone(), Some(Alias { target }));
            let start = l.annotated_start(d.span.start, &d.metadata);
            self.push(l.declaration(name, Kind::Typedef, None, Signature::None, start, d.span.end));
        }
        for did in variables {
            let d = ast.decl(did);
            let DeclKind::Variables(list) = &d.kind else { continue };
            let start = l.annotated_start(d.span.start, &d.metadata);
            for (i, v) in list.variables.iter().enumerate() {
                let name = l.name(v.name);
                self.union.add(&name);
                let start_ = if i == 0 { start } else { v.name.span.start };
                self.push(l.declaration(name, Kind::Variable, None, Signature::None, start_, l.variable_end(v)));
            }
        }
    }
}

/// As partes declaradas por uma unidade, resolvidas a partir do arquivo.
fn parts(unit: &ast::CompilationUnit, path: &Path) -> Vec<PathBuf> {
    let mut output = Vec::new();
    for d in &unit.directives {
        if let DirectiveKind::Part { uri } = &d.kind
            && let Some(relative) = dartforge_elements::load::string_lit_value(uri)
            && !relative.contains(':')
            && let Some(folder) = path.parent()
        {
            output.push(dartforge_elements::gerado::chave(&folder.join(relative)));
        }
    }
    output
}

fn is_part(unit: &ast::CompilationUnit) -> bool {
    unit.directives.iter().any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))
}

/// Indexa a biblioteca de `path`: `None` quando é parte ou não se lê.
fn index(path: &Path, read_: &Reader<'_>, uri_of: &dyn Fn(&Path) -> String) -> (Vec<(PathBuf, Fingerprint)>, Option<Library>) {
    let mut files = Vec::new();
    let mut union = NameUnion::empty();
    let mut types = LocalTypes::new();
    let mut units = Vec::new();
    // As unidades em pré-ordem: a que define, cada parte e as partes dela.
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
            comments: Comentarios::de(&source),
        };
        let mut collector = Collector { l: &read, output: Vec::new(), union: &mut union, types: &mut types };
        collector.unit(&analyzed.unit);
        let declarations = collector.output;
        let uri = uri_of(&file);
        units.push(Unit { uri, declarations });
        for p in parts(&analyzed.unit, &file).into_iter().rev() {
            pending.push(p);
        }
    }
    (files, Some(Library { union, units, types }))
}

impl Index {
    /// A biblioteca de `path`, do cache quando nenhum dos arquivos mudou.
    fn library(&mut self, path: &Path, read: &Reader<'_>, open: &HashMap<PathBuf, Fingerprint>, uri_of: &dyn Fn(&Path) -> String) -> Option<Arc<Library>> {
        let current = |p: &Path| open.get(&dartforge_elements::gerado::chave(p)).cloned().or_else(|| disk_fingerprint(p));
        if let Some(e) = self.entries.get(path)
            && e.files.iter().all(|(p, i)| current(p).as_ref() == Some(i))
        {
            return e.library.clone();
        }
        let (files, library) = index(path, read, uri_of);
        let library = library.map(Arc::new);
        self.entries.insert(path.to_path_buf(), Entry { files, library: library.clone() });
        library
    }

    /// As bibliotecas do SDK (`dartSdk.sdkLibraries`): as entradas do mapa
    /// `libraries` de `_internal/sdk_library_metadata/lib/libraries.dart`,
    /// na ordem escrita, com o caminho relativo a `lib/`.
    fn sdk_libraries(&mut self, lib: &Path) -> Vec<PathBuf> {
        if let Some((root, list)) = &self.sdk
            && root == lib
        {
            return list.clone();
        }
        let list = read_libraries_dart(lib);
        self.sdk = Some((lib.to_path_buf(), list.clone()));
        list
    }

    /// `FindDeclarations.compute` + o `_asSymbolInformation` do handler.
    /// `added` são os arquivos do projeto; `packages`, os `lib/` dos
    /// pacotes (na ordem do `package_config.json`); `open_texts`, a URI
    /// e o texto dos documentos abertos pelo caminho normalizado
    /// (`generated::key`);
    /// `kinds` os `SymbolKind` aceitos pelo cliente.
    pub(crate) fn search(
        &mut self,
        query: &str,
        added: &[PathBuf],
        sdk: Option<&Path>,
        packages: &[PathBuf],
        open_texts: &HashMap<PathBuf, (String, String)>,
        kinds: Option<&[u64]>,
    ) -> Vec<Value> {
        // Os documentos abertos valem pelo texto do editor (o overlay), com
        // a URI do cliente.
        let open: HashMap<PathBuf, Fingerprint> = open_texts.iter().map(|(k, (_, t))| (k.clone(), text_fingerprint(t))).collect();
        let read = |p: &Path| -> Option<(String, Fingerprint)> {
            let k = dartforge_elements::gerado::chave(p);
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
            match open_texts.get(&dartforge_elements::gerado::chave(p)) {
                Some((uri, _)) => uri.clone(),
                None => url::Url::from_file_path(p).map(|u| u.to_string()).unwrap_or_default(),
            }
        };
        // Os arquivos na ordem do `OwnedFiles` (adicionados, depois os
        // conhecidos que não foram adicionados).
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut include = |p: PathBuf, paths: &mut Vec<PathBuf>| {
            if seen.insert(dartforge_elements::gerado::chave(&p)) {
                paths.push(p);
            }
        };
        for p in added {
            include(p.clone(), &mut paths);
        }
        if let Some(lib) = sdk {
            for p in self.sdk_libraries(lib) {
                include(p, &mut paths);
            }
        }
        for folder in packages {
            let mut files = Vec::new();
            discover(folder, &mut files);
            for p in files {
                include(p, &mut paths);
            }
        }
        let libraries: Vec<Arc<Library>> = paths.iter().filter_map(|c| self.library(c, read, open, &uri_of)).collect();
        // Os `typedef` de todas as bibliotecas, para expandir os apelidos de
        // outra biblioteca (o primeiro de cada nome).
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

/// O `SymbolInformation` de uma declaração.
fn symbol(d: &Declaration, uri: &str, locals: &LocalTypes, global: &HashMap<String, Alias>, kinds: Option<&[u64]>) -> Value {
    let by_return = |return_: &Option<Summary>, augment: bool| {
        let p = return_.as_ref().map_or(Paren::None, |r| paren(r, &[], locals, global, 0));
        // `augment ` antes do retorno: o `(` nunca fica no começo.
        if augment && p == Paren::StartsWith { Paren::Contains } else { p }
    };
    // `parameters`: o texto exibido a partir do primeiro `(` (que não pode
    // ser o primeiro caractere); `()` quando é só a lista vazia.
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
        Signature::Executable { return_, empty, augment } => match by_return(return_, *augment) {
            Paren::StartsWith => "",
            Paren::Contains => "(…)",
            Paren::None if *empty => "()",
            Paren::None => "(…)",
        },
    };
    let accepts = |k: u64| kinds.map_or(k <= 18, |l| l.contains(&k));
    let kind = d.kind.preferences().iter().copied().find(|k| accepts(*k)).unwrap_or(19);
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

/// `discoverRecursively`: os `.dart` da pasta e das subpastas, na ordem do
/// sistema de arquivos (a mesma do `Directory.listSync` do Dart).
fn discover(folder: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
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

/// As entradas do mapa `libraries` do `libraries.dart` do SDK, na ordem
/// escrita: `'name': const LibraryInfo('path', …)`.
fn read_libraries_dart(lib: &Path) -> Vec<PathBuf> {
    let file = lib.join("_internal").join("sdk_library_metadata").join("lib").join("libraries.dart");
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let Some(start) = text.find("libraries = const {") else { return Vec::new() };
    let body = &text[start..];
    let end = body.find("\n};").unwrap_or(body.len());
    let body = &body[..end];
    let mut output = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("LibraryInfo(") {
        rest = &rest[i + "LibraryInfo(".len()..];
        let entry_text = rest.trim_start();
        let Some(quotes) = entry_text.chars().next().filter(|c| *c == '\'' || *c == '"') else { continue };
        let content = &entry_text[1..];
        let Some(f) = content.find(quotes) else { continue };
        let relative = &content[..f];
        output.push(dartforge_elements::gerado::chave(&lib.join(relative)));
    }
    output
}

/// Os diretórios `lib/` (o `packageUri`) dos pacotes do
/// `.dart_tool/package_config.json` de `root`, na ordem do arquivo.
pub(crate) fn package_folders(root: &Path) -> Vec<PathBuf> {
    let file = root.join(".dart_tool").join("package_config.json");
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    let Ok(json) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let Some(packages) = json.get("packages").and_then(Value::as_array) else { return Vec::new() };
    let Some(base) = file.parent() else { return Vec::new() };
    let resolve = |uri: &str, relative_to: &Path| -> Option<PathBuf> {
        if let Ok(u) = url::Url::parse(uri) {
            return u.to_file_path().ok();
        }
        Some(dartforge_elements::gerado::chave(&relative_to.join(uri.trim_end_matches('/'))))
    };
    let mut output = Vec::new();
    for p in packages {
        let Some(root_uri) = p.get("rootUri").and_then(Value::as_str) else { continue };
        let Some(package_root) = resolve(root_uri, base) else { continue };
        let package_uri = p.get("packageUri").and_then(Value::as_str).unwrap_or("lib/");
        if let Some(lib) = resolve(package_uri, &package_root) {
            output.push(lib);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn suffixes_by_return() {
        let locals = LocalTypes::new();
        let global = HashMap::new();
        let f = Declaration {
            name: "f".into(),
            kind: Kind::Function,
            container: None,
            signature: Signature::Executable { return_: Some(Summary::Registration), empty: true, augment: false },
            start: (0, 0),
            end: (0, 1),
        };
        assert_eq!(symbol(&f, "file:///a.dart", &locals, &global, None)["name"], "f");
        let g = Declaration { signature: Signature::Executable { return_: Some(Summary::Function(None)), empty: true, augment: false }, ..f.clone() };
        assert_eq!(symbol(&g, "file:///a.dart", &locals, &global, None)["name"], "f(…)");
        let h = Declaration { signature: Signature::Executable { return_: None, empty: true, augment: false }, ..f.clone() };
        assert_eq!(symbol(&h, "file:///a.dart", &locals, &global, None)["name"], "f()");
        let k = Declaration { kind: Kind::EnumConstant, signature: Signature::None, ..f };
        assert_eq!(symbol(&k, "file:///a.dart", &locals, &global, None)["kind"], 10);
    }
}
