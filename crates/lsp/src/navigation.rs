//! Safe navigation of directives with a relative or package URI. The
//! original analyzer records the literal's region in `ImportDirective`,
//! `ExportDirective` and `PartDirective` when the target exists.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{Ast, CompilationUnit, DeclKind, Directive, DirectiveKind, ExprKind, ForInTarget, ForInit, FunctionKind, MemberKind, Name, ParameterKind, StmtKind, StringLit, TypeKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use dartforge_elements::config::PackageConfig;
use url::Url;

use crate::{SyntacticAnalyzer, DocumentStore};

pub(super) enum Target {
    File(String),
    LocalName(LocalType),
}

pub(super) struct LocalType {
    pub declaration: Span,
    pub reference: Span,
    pub description: Option<String>,
    pub static_type: Option<String>,
}

/// Target for a relative/package URI literal or a single type of the file
/// itself. `dart:` libraries require the SDK mapping; a nonexistent path
/// produces no false navigation.
pub(super) fn target(
    current_uri: &str,
    text: &str,
    features: LibraryFeatures,
    offset: usize,
) -> Option<Target> {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    if let Some(target) = directive_target(current_uri, &parsed.unit, offset) {
        return Some(Target::File(target));
    }
    local_type(&parsed.unit, &parsed.ast, &names, offset)
        .or_else(|| top_variable(&parsed.unit, &parsed.ast, &names, offset))
        .or_else(|| top_function(&parsed.unit, &parsed.ast, &names, offset))
        .map(Target::LocalName)
}

/// Target of a directive literal (`import`, `export`, `part`, ...) under the
/// cursor, when the relative file exists on disk. It is one `stat` per
/// request, never a scan: the activation rule follows the analyzer's
/// directive navigation (there is only a target when the file exists).
fn directive_target(current_uri: &str, unit: &CompilationUnit, offset: usize) -> Option<String> {
    for directive in &unit.directives {
        let literal = match &directive.kind {
            DirectiveKind::Import { uri, .. }
            | DirectiveKind::Export { uri, .. }
            | DirectiveKind::Part { uri }
            | DirectiveKind::ImportAugment { uri }
            | DirectiveKind::AugmentLibrary { uri } => Some(uri),
            DirectiveKind::PartOf { uri, .. } => uri.as_ref(),
            DirectiveKind::Library { .. } => None,
        };
        let Some(literal) = literal else { continue };
        if literal.span.start <= offset && offset < literal.span.end {
            let base = Url::parse(current_uri).ok()?;
            if base.scheme() != "file" { return None; }
            return resolve(&base, literal);
        }
    }
    None
}

fn local_type(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    names: &Interner,
    offset: usize,
) -> Option<LocalType> {
    // Imports/exports/parts may bring in names that the AST of a single file
    // cannot distinguish. Until library resolution comes to the LSP, return
    // empty.
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. })) {
        return None;
    }
    let reference = ast.types.iter().find_map(|ty| {
        let TypeKind::Named { name, .. } = &ty.kind else { return None };
        if name.len() != 1 { return None; }
        let n = name[0];
        (n.span.start <= offset && offset < n.span.end).then_some(n)
    })?;
    let key = reference.sym;
    // A homonymous type parameter may shadow the top-level declaration.
    if ast.functions.iter().any(|f| f.type_params.iter().any(|p| p.name.sym == key))
        || ast.types.iter().any(|t| matches!(&t.kind, TypeKind::Function { type_params, .. }
            if type_params.iter().any(|p| p.name.sym == key)))
        || unit.declarations.iter().any(|id| {
            let parameters = match &ast.decl(*id).kind {
                DeclKind::Class(d) => d.type_params.as_ref(),
                DeclKind::Mixin(d) => d.type_params.as_ref(),
                DeclKind::Enum(d) => d.type_params.as_ref(),
                DeclKind::Extension(d) => d.type_params.as_ref(),
                DeclKind::ExtensionType(d) => d.type_params.as_ref(),
                DeclKind::Typedef(d) => d.type_params.as_ref(),
                _ => &[],
            };
            parameters.iter().any(|p| p.name.sym == key)
        })
    {
        return None;
    }
    let mut found = unit.declarations.iter().filter_map(|id| {
        let (name_, ty_, generico) = match &ast.decl(*id).kind {
            DeclKind::Class(d) => (d.name, "class", !d.type_params.is_empty()),
            DeclKind::Mixin(d) => (d.name, "mixin", !d.type_params.is_empty()),
            DeclKind::Enum(d) => (d.name, "enum", !d.type_params.is_empty()),
            DeclKind::ExtensionType(d) => (d.name, "extension type", !d.type_params.is_empty()),
            DeclKind::Typedef(d) => (d.name, "typedef", true),
            _ => return None,
        };
        (name_.sym == key).then_some((name_.span, ty_, generico))
    });
    let single = found.next()?;
    if found.next().is_some() { return None; }
    Some(LocalType {
        declaration: single.0,
        reference: reference.span,
        description: (!single.2).then(|| format!("{} {}", single.1, names.resolve(key))),
        static_type: None,
    })
}

/// Reference to a single top-level variable, without any homonymous
/// declaration that could shadow it. Only written primitive types give a
/// hover: the others need resolution of aliases, bounds and imports.
fn top_variable(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    names: &Interner,
    offset: usize,
) -> Option<LocalType> {
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        || !ast.patterns.is_empty()
    {
        return None;
    }
    let reference = reference_expr(ast, offset)?;
    let key = reference.sym;
    if shadowed(ast, key) { return None; }
    let mut found = Vec::new();
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in v.variables.iter().filter(|x| x.name.sym == key) {
                    found.push((var.name.span, v.ty));
                }
            }
            DeclKind::Class(d) if d.name.sym == key => return None,
            DeclKind::Mixin(d) if d.name.sym == key => return None,
            DeclKind::Enum(d) if d.name.sym == key => return None,
            DeclKind::ExtensionType(d) if d.name.sym == key => return None,
            DeclKind::Typedef(d) if d.name.sym == key => return None,
            DeclKind::Function(f) if ast.function(*f).name.is_some_and(|n| n.sym == key) => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == key) => return None,
            _ => {}
        }
    }
    if found.len() != 1 { return None; }
    let (declaration, ty) = found[0];
    let static_type = ty.and_then(|t| primitive_type(ast, names, t));
    let description = static_type.as_ref().map(|t| format!("{t} {}", names.resolve(key)));
    Some(LocalType { declaration, reference: reference.span, description, static_type })
}

pub(super) fn reference_expr(ast: &dartforge_frontend::ast::Ast, offset: usize) -> Option<Name> {
    ast.exprs.iter().find_map(|e| {
        let ExprKind::Identifier(n) = &e.kind else { return None };
        (n.span.start <= offset && offset < n.span.end).then_some(*n)
    })
}

pub(super) fn shadowed(ast: &dartforge_frontend::ast::Ast, key: dartforge_intern::SymbolId) -> bool {
    ast.functions.iter().any(|f| f.parameters.as_ref().is_some_and(|ps| ps.iter().any(|p| p.name.is_some_and(|n| n.sym == key))))
        || ast.members.iter().any(|m| match &m.kind {
            MemberKind::Field(v) => v.variables.iter().any(|x| x.name.sym == key),
            MemberKind::Method(f) => ast.function(*f).name.is_some_and(|n| n.sym == key),
            MemberKind::Constructor(c) => c.name.is_some_and(|n| n.sym == key),
        })
        || ast.stmts.iter().any(|s| match &s.kind {
            StmtKind::Variables(v) => v.variables.iter().any(|x| x.name.sym == key),
            StmtKind::Function(f) => ast.function(*f).name.is_some_and(|n| n.sym == key),
            StmtKind::For { init: Some(ForInit::Variables(v)), .. } =>
                v.variables.iter().any(|x| x.name.sym == key),
            StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => name.sym == key,
            StmtKind::Try { catches, .. } => catches.iter().any(|c|
                c.exception.is_some_and(|n| n.sym == key)
                    || c.stack_trace.is_some_and(|n| n.sym == key)),
            _ => false,
        })
}

pub(crate) fn primitive_type(ast: &dartforge_frontend::ast::Ast, names: &Interner, id: dartforge_frontend::ast::TypeId) -> Option<String> {
    let t = ast.ty(id);
    let TypeKind::Named { name, args } = &t.kind else { return None };
    if name.len() != 1 || !args.is_empty() { return None; }
    let base = names.resolve(name[0].sym);
    matches!(base, "int" | "double" | "num" | "bool" | "String" | "Object" | "dynamic")
        .then(|| format!("{base}{}", if t.nullable { "?" } else { "" }))
}

fn top_function(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    names: &Interner,
    offset: usize,
) -> Option<LocalType> {
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        || !ast.patterns.is_empty()
    { return None; }
    let reference = reference_expr(ast, offset)?;
    let key = reference.sym;
    if shadowed(ast, key) { return None; }
    let mut found = Vec::new();
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Function(f) => {
                let f = ast.function(*f);
                if let Some(name) = f.name.filter(|n| n.sym == key) {
                    found.push((name.span, f));
                }
            }
            DeclKind::Variables(v) if v.variables.iter().any(|x| x.name.sym == key) => return None,
            DeclKind::Class(d) if d.name.sym == key => return None,
            DeclKind::Mixin(d) if d.name.sym == key => return None,
            DeclKind::Enum(d) if d.name.sym == key => return None,
            DeclKind::ExtensionType(d) if d.name.sym == key => return None,
            DeclKind::Typedef(d) if d.name.sym == key => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == key) => return None,
            _ => {}
        }
    }
    if found.len() != 1 { return None; }
    let (declaration, function) = found[0];
    let description = (|| {
        if !function.type_params.is_empty() {
            return None;
        }
        let return_ = function.return_type.and_then(|t| {
            if matches!(&ast.ty(t).kind, TypeKind::Void) {
                Some("void".to_string())
            } else {
                primitive_type(ast, names, t)
            }
        })?;
        if function.kind == FunctionKind::Getter {
            return function.parameters.is_none().then(|| (format!("{return_} get {}", names.resolve(key)), Some(return_)));
        }
        if function.kind != FunctionKind::Function { return None; }
        let parameters = function.parameters.as_ref()?;
        // With 3+ parameters, the analyzer switches to a multi-line layout.
        if parameters.len() > 2 { return None; }
        let mut descriptions = Vec::new();
        for p in parameters.iter() {
            if p.kind != ParameterKind::Required || p.covariant || p.final_ || p.var_ || p.const_
                || p.this_ || p.super_ || p.default_value.is_some()
                || !p.function_type_params.is_empty() || p.function_parameters.is_some()
            { return None; }
            let ty = primitive_type(ast, names, p.ty?)?;
            let name = names.resolve(p.name?.sym);
            descriptions.push(format!("{ty} {name}"));
        }
        Some((format!("{return_} {}({})", names.resolve(key), descriptions.join(", ")), None))
    })();
    let (description, static_type) = description.map_or((None, None), |(d, t)| (Some(d), t));
    Some(LocalType { declaration, reference: reference.span, description, static_type })
}

fn resolve(base: &Url, literal: &StringLit) -> Option<String> {
    let value = literal.constant_value()?;
    let path = value.as_str()?;
    if path.starts_with("dart:") {
        return None;
    }
    let target = if path.starts_with("package:") {
        let uri = Url::parse(path).ok()?;
        if uri.query().is_some() || uri.fragment().is_some() { return None; }
        let origin = base.to_file_path().ok()?;
        let config = PackageConfig::discover(&origin)?;
        let config = PackageConfig::load(&config).ok()?;
        let name = path.strip_prefix("package:")?.split('/').next()?;
        // The compilation resolver falls back to `references/pub`; in the LSP,
        // navigation must follow only the mapping of the open project.
        if !config.packages.contains_key(name) { return None; }
        config.resolve_package_uri(path).ok()?
    } else {
        let resolved = base.join(path).ok()?;
        if resolved.scheme() != "file" || resolved.query().is_some() || resolved.fragment().is_some() {
            return None;
        }
        resolved.to_file_path().ok()?
    };
    if !target.is_file() {
        return None;
    }
    let canonical = std::fs::canonicalize(target).ok()?;
    Url::from_file_path(canonical).ok().map(|u| u.to_string())
}

/// References in the document itself for the safe cases of [`target`].
///
/// Returns the declaration first and then the uses in offset order, all in
/// the same file. `None` means "I don't know" (imports, patterns, shadows,
/// duplicate names or cursor outside a known name) — never "there are
/// none". Across files it waits for library and element resolution.
///
/// The tree is temporary (a new `Interner` per call), as in [`target`]:
/// nothing is retained between edits, keeping the memory plateau.
pub(super) fn references(
    _current_uri: &str,
    text: &str,
    features: LibraryFeatures,
    offset: usize,
) -> Option<Vec<Span>> {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    let unit = &parsed.unit;
    let ast = &parsed.ast;
    // Only the file itself, without names coming from outside; patterns bind
    // names that the AST of a single file cannot distinguish.
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        || !ast.patterns.is_empty()
    {
        return None;
    }
    // Finds the key under the cursor: type reference, declared type name,
    // expression identifier or declared top-level value name.
    let key = key_at_cursor(unit, ast, offset)?;
    // No shadows in any scope the AST can see.
    if shadowed(ast, key) {
        return None;
    }
    if has_homonymous_type_parameter(unit, ast, key) {
        return None;
    }
    let type_decl = type_declaration(unit, ast, key);
    let value_decl = value_declaration(unit, ast, key);
    match (type_decl, value_decl) {
        (Some(decl), None) => {
            let mut uses = vec![decl];
            for ty in &ast.types {
                let TypeKind::Named { name, .. } = &ty.kind else { continue };
                if name.len() != 1 || name[0].sym != key {
                    continue;
                }
                if name[0].span == decl {
                    continue;
                }
                uses.push(name[0].span);
            }
            uses[1..].sort_by_key(|s| s.start);
            let mut output = vec![uses[0]];
            output.extend(uses[1..].iter().copied());
            Some(output)
        }
        (None, Some(decl)) => {
            let mut uses = vec![decl];
            for e in &ast.exprs {
                let ExprKind::Identifier(n) = &e.kind else { continue };
                if n.sym != key {
                    continue;
                }
                uses.push(n.span);
            }
            uses[1..].sort_by_key(|s| s.start);
            Some(uses)
        }
        // Zero or two namespaces with the same name: conservative, no answer.
        _ => None,
    }
}

/// Symbol under the cursor, whether in a reference or in the declared name itself.
fn key_at_cursor(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    offset: usize,
) -> Option<dartforge_intern::SymbolId> {
    for ty in &ast.types {
        let TypeKind::Named { name, .. } = &ty.kind else { continue };
        if name.len() != 1 {
            continue;
        }
        if name[0].span.start <= offset && offset < name[0].span.end {
            return Some(name[0].sym);
        }
    }
    if let Some(sym) = declared_type_name_at_cursor(unit, ast, offset) {
        return Some(sym);
    }
    for e in &ast.exprs {
        let ExprKind::Identifier(n) = &e.kind else { continue };
        if n.span.start <= offset && offset < n.span.end {
            return Some(n.sym);
        }
    }
    declared_value_name_at_cursor(unit, ast, offset)
}

/// Declared type name (class, mixin, enum, extension type, typedef).
fn declared_type_name_at_cursor(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    offset: usize,
) -> Option<dartforge_intern::SymbolId> {
    for id in &unit.declarations {
        let name = match &ast.decl(*id).kind {
            DeclKind::Class(d) => Some(d.name),
            DeclKind::Mixin(d) => Some(d.name),
            DeclKind::Enum(d) => Some(d.name),
            DeclKind::ExtensionType(d) => Some(d.name),
            DeclKind::Typedef(d) => Some(d.name),
            _ => None,
        };
        if let Some(n) = name
            && n.span.start <= offset
            && offset < n.span.end
        {
            return Some(n.sym);
        }
    }
    None
}

/// Declared top-level value name (variable, function, getter, setter).
fn declared_value_name_at_cursor(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    offset: usize,
) -> Option<dartforge_intern::SymbolId> {
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in &v.variables {
                    if var.name.span.start <= offset && offset < var.name.span.end {
                        return Some(var.name.sym);
                    }
                }
            }
            DeclKind::Function(f) => {
                if let Some(n) = ast.function(*f).name
                    && n.span.start <= offset
                    && offset < n.span.end
                {
                    return Some(n.sym);
                }
            }
            _ => {}
        }
    }
    None
}

/// Some homonymous type parameter may shadow the top-level declaration.
fn has_homonymous_type_parameter(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    key: dartforge_intern::SymbolId,
) -> bool {
    if ast.functions.iter().any(|f| f.type_params.iter().any(|p| p.name.sym == key)) {
        return true;
    }
    if ast.types.iter().any(|t| matches!(&t.kind, TypeKind::Function { type_params, .. }
        if type_params.iter().any(|p| p.name.sym == key)))
    {
        return true;
    }
    unit.declarations.iter().any(|id| {
        let parameters = match &ast.decl(*id).kind {
            DeclKind::Class(d) => d.type_params.as_ref(),
            DeclKind::Mixin(d) => d.type_params.as_ref(),
            DeclKind::Enum(d) => d.type_params.as_ref(),
            DeclKind::Extension(d) => d.type_params.as_ref(),
            DeclKind::ExtensionType(d) => d.type_params.as_ref(),
            DeclKind::Typedef(d) => d.type_params.as_ref(),
            _ => &[],
        };
        parameters.iter().any(|p| p.name.sym == key)
    })
}

/// Single type declaration with the name (span of the declared name).
fn type_declaration(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    key: dartforge_intern::SymbolId,
) -> Option<Span> {
    let mut found = unit.declarations.iter().filter_map(|id| {
        let name = match &ast.decl(*id).kind {
            DeclKind::Class(d) => Some(d.name),
            DeclKind::Mixin(d) => Some(d.name),
            DeclKind::Enum(d) => Some(d.name),
            DeclKind::ExtensionType(d) => Some(d.name),
            DeclKind::Typedef(d) => Some(d.name),
            _ => None,
        };
        name.filter(|n| n.sym == key).map(|n| n.span)
    });
    let single = found.next()?;
    found.next().is_none().then_some(single)
}

/// Single top-level value declaration with the name (span of the declared name).
fn value_declaration(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    key: dartforge_intern::SymbolId,
) -> Option<Span> {
    let mut found = Vec::new();
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in v.variables.iter().filter(|x| x.name.sym == key) {
                    found.push(var.name.span);
                }
            }
            DeclKind::Function(f) => {
                if let Some(n) = ast.function(*f).name.filter(|n| n.sym == key) {
                    found.push(n.span);
                }
            }
            DeclKind::Class(d) if d.name.sym == key => return None,
            DeclKind::Mixin(d) if d.name.sym == key => return None,
            DeclKind::Enum(d) if d.name.sym == key => return None,
            DeclKind::ExtensionType(d) if d.name.sym == key => return None,
            DeclKind::Typedef(d) if d.name.sym == key => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == key) => return None,
            _ => {}
        }
    }
    (found.len() == 1).then(|| found[0])
}

// ---------------------------------------------------------------------------
// Across open documents: the smallest useful step of semantic resolution.
// ---------------------------------------------------------------------------

/// Namespace of the top-level declaration: types and values do not collide,
/// so each search carries its own so as not to mix `class Caixa` with
/// `int Caixa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    Type,
    Value,
}

/// Top-level symbol resolved across the open documents.
struct Symbol {
    owner: String,
    space: Space,
    name: String,
    declaration: Span,
}

/// Name under the cursor with its range: the key (a `SymbolId` is only valid
/// in its own interner) and the span of the reference in the current file.
/// Covers the same places as [`key_at_cursor`] (reference or declared name);
/// hover returns this span as `range`, in the cursor's file.
fn key_and_span_at_cursor(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    offset: usize,
) -> Option<(dartforge_intern::SymbolId, Span)> {
    for ty in &ast.types {
        let TypeKind::Named { name, .. } = &ty.kind else { continue };
        if name.len() != 1 {
            continue;
        }
        if name[0].span.start <= offset && offset < name[0].span.end {
            return Some((name[0].sym, name[0].span));
        }
    }
    for id in &unit.declarations {
        let name_ = match &ast.decl(*id).kind {
            DeclKind::Class(d) => Some(d.name),
            DeclKind::Mixin(d) => Some(d.name),
            DeclKind::Enum(d) => Some(d.name),
            DeclKind::ExtensionType(d) => Some(d.name),
            DeclKind::Typedef(d) => Some(d.name),
            _ => None,
        };
        if let Some(n) = name_
            && n.span.start <= offset
            && offset < n.span.end
        {
            return Some((n.sym, n.span));
        }
    }
    for e in &ast.exprs {
        let ExprKind::Identifier(n) = &e.kind else { continue };
        if n.span.start <= offset && offset < n.span.end {
            return Some((n.sym, n.span));
        }
    }
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in &v.variables {
                    if var.name.span.start <= offset && offset < var.name.span.end {
                        return Some((var.name.sym, var.name.span));
                    }
                }
            }
            DeclKind::Function(f) => {
                if let Some(n) = ast.function(*f).name
                    && n.span.start <= offset
                    && offset < n.span.end
                {
                    return Some((n.sym, n.span));
                }
            }
            _ => {}
        }
    }
    None
}

/// An analyzed file with its interner: the `SymbolId`s are only valid inside
/// it, so across files the text is compared (`lookup`/`resolve`).
struct File {
    names: Interner,
    unit: CompilationUnit,
    ast: Ast,
}

/// Analyzes the text with the file's own features and discards the
/// diagnostic: only the tree matters here, transient to the request as in
/// [`references`].
fn analyze(text: &str, features: LibraryFeatures) -> File {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    File { names, unit: parsed.unit, ast: parsed.ast }
}

/// Class of a directive for resolution across open documents.
enum DirectiveClass {
    Library,
    /// Simple relative import: no prefix, combinators, `deferred` or condition,
    /// with a constant literal outside `dart:`/`package:`.
    SimpleImport(String),
    /// Everything else: `export`, `part`, `part of`, augmentations, prefix,
    /// `show`/`hide`, `dart:`/`package:`, interpolation. Brings names that only
    /// library resolution can distinguish: a file with one of these is never an
    /// owner nor a witness, and a cursor on one of them is "I don't know".
    Complex,
}

fn classify(directive: &Directive) -> DirectiveClass {
    match &directive.kind {
        DirectiveKind::Library { .. } => DirectiveClass::Library,
        DirectiveKind::Import { uri, configurations, deferred, prefix, combinators } => {
            if *deferred || prefix.is_some() || !combinators.is_empty() || !configurations.is_empty() {
                return DirectiveClass::Complex;
            }
            match uri.constant_value().and_then(|v| v.as_str().map(str::to_string)) {
                Some(path)
                    if !path.starts_with("dart:")
                        && !path.starts_with("package:") =>
                {
                    DirectiveClass::SimpleImport(path)
                }
                _ => DirectiveClass::Complex,
            }
        }
        _ => DirectiveClass::Complex,
    }
}

/// Resolves a relative import against the base URI without touching the
/// disk: joins the segments (`Url::join`) and returns the normalized URI.
/// `None` for `dart:`/`package:`, a base outside `file:`, a query or a
/// fragment. Differs from [`resolve`] on purpose: no `canonicalize`, no
/// `is_file` — the question here is "which open document does this URI
/// name", never "what exists on disk".
fn resolve_open(base_uri: &str, path: &str) -> Option<String> {
    if path.starts_with("dart:") || path.starts_with("package:") {
        return None;
    }
    let base = Url::parse(base_uri).ok()?;
    if base.scheme() != "file" {
        return None;
    }
    let resolved = base.join(path).ok()?;
    if resolved.scheme() != "file" || resolved.query().is_some() || resolved.fragment().is_some() {
        return None;
    }
    Some(resolved.to_string())
}

/// Resolves the symbol under the cursor to the open document that declares it.
///
/// Local case (no directives besides `library`): the file itself, as in
/// [`references`]. Imported case (only simple imports, a name without a
/// local declaration): the single imported open document that declares the
/// name alone in one namespace, itself without directives that bring other
/// names. Everything else — complex directive, patterns, shadows, homonymous
/// type parameter, zero or several owners — is `None` ("I don't know",
/// never a wrong answer).
fn resolve_symbol(
    documents: &DocumentStore,
    current_uri: &str,
    offset: usize,
    analyzer: &mut SyntacticAnalyzer,
) -> Option<(Symbol, Span)> {
    let text = documents.get(current_uri)?.to_string();
    let file = analyze(&text, analyzer.features(current_uri, &text));
    if !file.ast.patterns.is_empty() {
        return None;
    }
    let (key, reference) = key_and_span_at_cursor(&file.unit, &file.ast, offset)?;
    if shadowed(&file.ast, key)
        || has_homonymous_type_parameter(&file.unit, &file.ast, key)
    {
        return None;
    }
    let name = file.names.resolve(key).to_string();
    let type_decl = type_declaration(&file.unit, &file.ast, key);
    let value_decl = value_declaration(&file.unit, &file.ast, key);
    let mut simple = Vec::new();
    let mut simple_only = true;
    for directive in &file.unit.directives {
        match classify(directive) {
            DirectiveClass::Library => {}
            DirectiveClass::SimpleImport(path) => simple.push(path),
            DirectiveClass::Complex => simple_only = false,
        }
    }
    if simple.is_empty() && simple_only {
        return match (type_decl, value_decl) {
            (Some(declaration), None) => Some((
                Symbol {
                    owner: current_uri.to_string(),
                    space: Space::Type,
                    name,
                    declaration,
                },
                reference,
            )),
            (None, Some(declaration)) => Some((
                Symbol {
                    owner: current_uri.to_string(),
                    space: Space::Value,
                    name,
                    declaration,
                },
                reference,
            )),
            _ => None,
        };
    }
    if !simple_only || type_decl.is_some() || value_decl.is_some() {
        return None;
    }
    let mut candidates = Vec::new();
    for path in &simple {
        if let Some(uri) = resolve_open(current_uri, path)
            && !candidates.contains(&uri)
        {
            candidates.push(uri);
        }
    }
    single_owner(documents, analyzer, &candidates, &name).map(|s| (s, reference))
}

/// The single open candidate that declares `name` alone in one namespace,
/// without directives that bring other names, patterns, shadows or a
/// homonymous type parameter. Zero (owner closed or on disk) or several:
/// `None`.
fn single_owner(
    documents: &DocumentStore,
    analyzer: &mut SyntacticAnalyzer,
    candidates: &[String],
    name: &str,
) -> Option<Symbol> {
    let mut found = None;
    for candidate in candidates {
        let Some(text) = documents.get(candidate).map(str::to_string) else {
            continue;
        };
        let file = analyze(&text, analyzer.features(candidate, &text));
        if !file.ast.patterns.is_empty()
            || file.unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        {
            continue;
        }
        let Some(key) = file.names.lookup(name) else {
            continue;
        };
        if shadowed(&file.ast, key)
            || has_homonymous_type_parameter(&file.unit, &file.ast, key)
        {
            continue;
        }
        let owner = match (
            type_declaration(&file.unit, &file.ast, key),
            value_declaration(&file.unit, &file.ast, key),
        ) {
            (Some(declaration), None) => Symbol {
                owner: candidate.clone(),
                space: Space::Type,
                name: name.to_string(),
                declaration,
            },
            (None, Some(declaration)) => Symbol {
                owner: candidate.clone(),
                space: Space::Value,
                name: name.to_string(),
                declaration,
            },
            // Does not declare it, or declares it in both namespaces: not a clean owner.
            _ => continue,
        };
        if found.is_some() {
            return None;
        }
        found = Some(owner);
    }
    found
}

/// Definition across open documents: a directive literal follows the disk
/// rule ([`directive_target`], one `stat`); a name follows
/// [`resolve_symbol`] (only open documents, never the disk). `None` is "I
/// don't know".
pub(super) fn definition_in(
    documents: &DocumentStore,
    current_uri: &str,
    offset: usize,
    analyzer: &mut SyntacticAnalyzer,
) -> Option<(String, Option<Span>)> {
    let text = documents.get(current_uri)?.to_string();
    let features = analyzer.features(current_uri, &text);
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(&text, &mut names, features);
    if let Some(target) = directive_target(current_uri, &parsed.unit, offset) {
        return Some((target, None));
    }
    let symbol = resolve_symbol(documents, current_uri, offset, analyzer)?.0;
    Some((symbol.owner, Some(symbol.declaration)))
}

/// References across open documents through [`resolve_symbol`]: the
/// declaration first, then the uses sorted by (URI, offset), each file
/// analyzed and released before the next one — like `workspace/symbol`,
/// nothing is retained between requests and the `DocumentStore` stays the
/// sole owner of the texts.
///
/// Uses of a file only count when it imports the owner through a simple
/// import and nothing else can bring the name: complex directive, pattern,
/// shadow, homonymous local declaration, a simple import outside the open
/// documents (whose content the request cannot read without scanning the
/// disk) or another open document that declares the name — any doubt skips
/// the file, never invents a use nor returns a partial list as complete:
/// the caller cannot tell the difference.
pub(super) fn references_in(
    documents: &DocumentStore,
    current_uri: &str,
    offset: usize,
    analyzer: &mut SyntacticAnalyzer,
) -> Option<Vec<(String, Span)>> {
    let symbol = resolve_symbol(documents, current_uri, offset, analyzer)?.0;
    let mut outputs = vec![(symbol.owner.clone(), symbol.declaration)];
    let mut uris: Vec<String> = documents.uris().map(str::to_string).collect();
    uris.sort_unstable();
    for uri in uris {
        let uses = if uri == symbol.owner {
            owner_uses(documents, analyzer, &symbol)?
        } else {
            let Some(uses) = uses_if_imports(documents, analyzer, &uri, &symbol) else {
                continue;
            };
            uses
        };
        outputs.extend(uses.into_iter().map(|span| (uri.clone(), span)));
    }    outputs[1..].sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.start.cmp(&b.1.start)));
    Some(outputs)
}

/// Uses in the owner document, without the declaration. `None` when the
/// owner stopped being readable between resolution and collection (does not
/// happen in synchronous dispatch; the conservative `None` avoids inventing
/// a location).
fn owner_uses(
    documents: &DocumentStore,
    analyzer: &mut SyntacticAnalyzer,
    symbol: &Symbol,
) -> Option<Vec<Span>> {
    let text = documents.get(&symbol.owner)?.to_string();
    let file = analyze(&text, analyzer.features(&symbol.owner, &text));
    let key = file.names.lookup(&symbol.name)?;
    Some(uses_without_declaration(&file, key, symbol.space, symbol.declaration))
}

/// Uses in a file that imports the owner through a simple import. `None`
/// skips the file (doubt, or name absent for another reason); `Some` — even
/// empty, when it imports but does not use — contributes.
fn uses_if_imports(
    documents: &DocumentStore,
    analyzer: &mut SyntacticAnalyzer,
    uri: &str,
    symbol: &Symbol,
) -> Option<Vec<Span>> {
    let text = documents.get(uri)?.to_string();
    let file = analyze(&text, analyzer.features(uri, &text));
    if !file.ast.patterns.is_empty() {
        return None;
    }
    let mut owner_imports = false;
    for directive in &file.unit.directives {
        match classify(directive) {
            DirectiveClass::Library => {}
            DirectiveClass::SimpleImport(path) => {
                let target = resolve_open(uri, &path)?;
                if target == symbol.owner {
                    owner_imports = true;
                } else if documents.get(&target).is_some() {
                    if declares_name(documents, analyzer, &target, &symbol.name) {
                        return None;
                    }
                } else {
                    // Outside the open documents: it may declare the same name and the request
                    // does not read the disk on each keystroke. Skips the file instead of
                    // assigning the uses to the wrong owner.
                    return None;
                }
            }
            DirectiveClass::Complex => return None,
        }
    }
    if !owner_imports {
        return None;
    }
    let Some(key) = file.names.lookup(&symbol.name) else {
        return Some(Vec::new());
    };
    if shadowed(&file.ast, key) {
        return None;
    }
    if symbol.space == Space::Type
        && has_homonymous_type_parameter(&file.unit, &file.ast, key)
    {
        return None;
    }
    if type_declaration(&file.unit, &file.ast, key).is_some()
        || value_declaration(&file.unit, &file.ast, key).is_some()
    {
        // Homonymous local declaration: the uses target it, not the owner.
        return None;
    }
    Some(uses_without_declaration(&file, key, symbol.space, symbol.declaration))
}

/// True when the open document declares `name` in any namespace (type or
/// value): an importer that also imports it has no single owner.
fn declares_name(
    documents: &DocumentStore,
    analyzer: &mut SyntacticAnalyzer,
    uri: &str,
    name: &str,
) -> bool {
    let Some(text) = documents.get(uri).map(str::to_string) else {
        return false;
    };
    let file = analyze(&text, analyzer.features(uri, &text));
    let Some(key) = file.names.lookup(name) else {
        return false;
    };
    type_declaration(&file.unit, &file.ast, key).is_some()
        || value_declaration(&file.unit, &file.ast, key).is_some()
}

/// Uses of the name in the file, without the owner's declaration.
fn uses_without_declaration(
    file: &File,
    key: dartforge_intern::SymbolId,
    space: Space,
    declaration: Span,
) -> Vec<Span> {
    let mut uses = Vec::new();
    match space {
        Space::Type => {
            for ty in &file.ast.types {
                let TypeKind::Named { name, .. } = &ty.kind else { continue };
                if name.len() != 1 || name[0].sym != key || name[0].span == declaration {
                    continue;
                }
                uses.push(name[0].span);
            }
        }
        Space::Value => {
            for e in &file.ast.exprs {
                let ExprKind::Identifier(n) = &e.kind else { continue };
                if n.sym != key {
                    continue;
                }
                uses.push(n.span);
            }
        }
    }
    uses.sort_by_key(|s| s.start);
    uses
}

/// Hover across open documents: resolves like [`resolve_symbol`] and formats
/// the description from the owner document, with the same rules as the local
/// hover (non-generic types, variables of a written primitive type,
/// functions of up to two typed positionals, getters with a primitive
/// return). The returned `range` is that of the reference under the cursor,
/// in the current file.
///
/// Bench: owner not open, ambiguous symbol (zero or two owners), complex
/// directive (`prefix`, `show`/`hide`, ...), shadow, pattern or a signature
/// that requires full formatting → `None` (empty hover), never a wrong
/// text. Nothing is retained between requests: each tree is transient, as in
/// [`references_in`]; only the open documents are read, never the disk.
pub(super) fn hover_in(
    documents: &DocumentStore,
    current_uri: &str,
    offset: usize,
    analyzer: &mut SyntacticAnalyzer,
) -> Option<(Span, String, Option<String>)> {
    let (symbol, reference) = resolve_symbol(documents, current_uri, offset, analyzer)?;
    let (description, ty) = description_in_owner(documents, analyzer, &symbol)?;
    Some((reference, description, ty))
}

/// Description of the symbol from the owner, with the same format as the
/// local hover. `None` when the signature cannot be shown faithfully
/// (generic, typedef, non-primitive type, optional/named parameters,
/// inferred return, setter): the caller returns an empty hover.
fn description_in_owner(
    documents: &DocumentStore,
    analyzer: &mut SyntacticAnalyzer,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let text = documents.get(&symbol.owner)?.to_string();
    let file = analyze(&text, analyzer.features(&symbol.owner, &text));
    let key = file.names.lookup(&symbol.name)?;
    match symbol.space {
        Space::Type => type_description_in_file(&file, key).map(|d| (d, None)),
        Space::Value => value_description_in_file(&file, key),
    }
}

/// `class C` / `enum E` / `mixin M` / `extension type X`, when not generic;
/// same rule as [`local_type`].
fn type_description_in_file(
    file: &File,
    key: dartforge_intern::SymbolId,
) -> Option<String> {
    let mut found = file.unit.declarations.iter().filter_map(|id| {
        let (name, ty, generico) = match &file.ast.decl(*id).kind {
            DeclKind::Class(d) => (d.name, "class", !d.type_params.is_empty()),
            DeclKind::Mixin(d) => (d.name, "mixin", !d.type_params.is_empty()),
            DeclKind::Enum(d) => (d.name, "enum", !d.type_params.is_empty()),
            DeclKind::ExtensionType(d) => (d.name, "extension type", !d.type_params.is_empty()),
            DeclKind::Typedef(d) => (d.name, "typedef", true),
            _ => return None,
        };
        (name.sym == key).then_some((name.span, ty, generico))
    });
    let single = found.next()?;
    if found.next().is_some() {
        return None;
    }
    (!single.2).then(|| format!("{} {}", single.1, file.names.resolve(key)))
}

/// Top-level variable with a written primitive type, or a top-level
/// function/getter with a faithful signature; same rules as
/// [`top_variable`] and [`top_function`].
fn value_description_in_file(
    file: &File,
    key: dartforge_intern::SymbolId,
) -> Option<(String, Option<String>)> {
    let mut variables = Vec::new();
    let mut has_function = false;
    for id in &file.unit.declarations {
        match &file.ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in v.variables.iter().filter(|x| x.name.sym == key) {
                    variables.push((var.name.span, v.ty));
                }
            }
            DeclKind::Class(d) if d.name.sym == key => return None,
            DeclKind::Mixin(d) if d.name.sym == key => return None,
            DeclKind::Enum(d) if d.name.sym == key => return None,
            DeclKind::ExtensionType(d) if d.name.sym == key => return None,
            DeclKind::Typedef(d) if d.name.sym == key => return None,
            DeclKind::Function(f)
                if file.ast.function(*f).name.is_some_and(|n| n.sym == key) =>
            {
                // Function/getter name: not a variable (as in [`top_variable`]); falls
                // through to the signature formatting below (as in [`top_function`]).
                has_function = true;
            }
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == key) => return None,
            _ => {}
        }
    }
    if !variables.is_empty() {
        if has_function || variables.len() != 1 {
            return None;
        }
        let ty = primitive_type(&file.ast, &file.names, variables[0].1?)?;
        let description = format!("{ty} {}", file.names.resolve(key));
        return Some((description, Some(ty)));
    }
    if !has_function {
        return None;
    }
    let mut functions = Vec::new();
    for id in &file.unit.declarations {
        match &file.ast.decl(*id).kind {
            DeclKind::Function(f) => {
                let f = file.ast.function(*f);
                if let Some(name) = f.name.filter(|n| n.sym == key) {
                    functions.push((name.span, f));
                }
            }
            DeclKind::Variables(v)
                if v.variables.iter().any(|x| x.name.sym == key) =>
            {
                return None;
            }
            DeclKind::Class(d) if d.name.sym == key => return None,
            DeclKind::Mixin(d) if d.name.sym == key => return None,
            DeclKind::Enum(d) if d.name.sym == key => return None,
            DeclKind::ExtensionType(d) if d.name.sym == key => return None,
            DeclKind::Typedef(d) if d.name.sym == key => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == key) => return None,
            _ => {}
        }
    }
    if functions.len() != 1 {
        return None;
    }
    let function = functions[0].1;
    let (description, ty) = (|| {
        if !function.type_params.is_empty() {
            return None;
        }
        let return_ = function.return_type.and_then(|t| {
            if matches!(&file.ast.ty(t).kind, TypeKind::Void) {
                Some("void".to_string())
            } else {
                primitive_type(&file.ast, &file.names, t)
            }
        })?;
        if function.kind == FunctionKind::Getter {
            return function
                .parameters
                .is_none()
                .then(|| (format!("{return_} get {}", file.names.resolve(key)), Some(return_)));
        }
        if function.kind != FunctionKind::Function {
            return None;
        }
        let parameters = function.parameters.as_ref()?;
        if parameters.len() > 2 {
            return None;
        }
        let mut descriptions = Vec::new();
        for p in parameters.iter() {
            if p.kind != ParameterKind::Required
                || p.covariant
                || p.final_
                || p.var_
                || p.const_
                || p.this_
                || p.super_
                || p.default_value.is_some()
                || !p.function_type_params.is_empty()
                || p.function_parameters.is_some()
            {
                return None;
            }
            let ty = primitive_type(&file.ast, &file.names, p.ty?)?;
            let name = file.names.resolve(p.name?.sym);
            descriptions.push(format!("{ty} {name}"));
        }
        Some((
            format!(
                "{return_} {}({})",
                file.names.resolve(key),
                descriptions.join(", ")
            ),
            None,
        ))
    })()?;
    Some((description, ty))
}
