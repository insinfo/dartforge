//! Navegação segura de diretivas com URI relativa ou de pacote.
//! O analyzer original registra a região do literal em `ImportDirective`,
//! `ExportDirective` e `PartDirective` quando o destino existe.

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

/// Destino para um literal de URI relativa/de pacote ou tipo único do próprio
/// arquivo. Bibliotecas `dart:` exigem o mapeamento do SDK; caminho inexistente
/// não gera navegação falsa.
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

/// Destino de um literal de diretiva (`import`, `export`, `part`, ...) sob o
/// cursor, quando o arquivo relativo existe no disco. É um `stat` por pedido,
/// nunca varredura: a regra de ativação segue a navegação de diretivas do
/// analyzer (só existe alvo quando o arquivo existe).
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
    // Imports/exports/parts podem trazer nomes que o AST de um arquivo só não
    // distingue. Até a resolução de biblioteca entrar no LSP, devolva vazio.
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
    // Um parâmetro de tipo homônimo pode sombrear a declaração de topo.
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

/// Referência a variável de topo única, sem qualquer declaração homônima que
/// possa sombreá-la. Só os tipos primitivos escritos dão hover: os outros
/// precisam da resolução de aliases, limites e imports.
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
        // Com 3+ parâmetros, o analyzer muda para layout multilinha.
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
        // O resolver de compilação tem fallback para `references/pub`; no LSP
        // a navegação deve seguir apenas o mapeamento do projeto aberto.
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

/// Referências no próprio documento para os casos seguros de [`target`].
///
/// Devolve a declaração primeiro e depois os usos em ordem de offset, todos
/// no mesmo arquivo. `None` significa "não sei" (imports, padrões, sombras,
/// nomes duplicados ou cursor fora de nome conhecido) — nunca "não há".
/// Entre arquivos aguarda a resolução de bibliotecas e elementos.
///
/// A árvore é temporária (um `Interner` novo por chamada), como em
/// [`target`]: nada é retido entre edições, mantendo o platô de memória.
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
    // Só o próprio arquivo, sem nomes vindos de fora; padrões ligam nomes
    // que o AST de um arquivo só não distingue.
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        || !ast.patterns.is_empty()
    {
        return None;
    }
    // Descobre a chave sob o cursor: referência de tipo, nome declarado de
    // tipo, identificador de expressão ou nome declarado de valor de topo.
    let key = key_at_cursor(unit, ast, offset)?;
    // Sem sombras em qualquer escopo que o AST enxerga.
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
        // Zero ou dois namespaces com o mesmo nome: conservador, sem resposta.
        _ => None,
    }
}

/// Símbolo sob o cursor, seja numa referência ou no próprio nome declarado.
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

/// Nome de tipo declarado (classe, mixin, enum, extension type, typedef).
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

/// Nome de valor de topo declarado (variável, função, getter, setter).
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

/// Algum parâmetro de tipo homônimo pode sombrear a declaração de topo.
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

/// Declaração de tipo única com o nome (span do nome declarado).
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

/// Declaração de valor de topo única com o nome (span do nome declarado).
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
// Entre documentos abertos: o menor passo útil da resolução semântica.
// ---------------------------------------------------------------------------

/// Espaço de nomes da declaração de topo: tipos e valores não colidem, então
/// cada busca carrega o seu para não misturar `class Caixa` com `int Caixa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    Type,
    Value,
}

/// Símbolo de topo resolvido entre os documentos abertos.
struct Symbol {
    owner: String,
    space: Space,
    name: String,
    declaration: Span,
}

/// Nome sob o cursor com o seu intervalo: a chave (`SymbolId` só vale no
/// próprio internador) e o span da referência no arquivo vigente. Cobre os
/// mesmos lugares que [`key_at_cursor`] (referência ou nome declarado);
/// o hover devolve este span como `range`, no arquivo do cursor.
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

/// Um arquivo analisado com o seu internador: os `SymbolId` valem só dentro
/// dele, então entre arquivos compara-se o texto (`lookup`/`resolve`).
struct File {
    names: Interner,
    unit: CompilationUnit,
    ast: Ast,
}

/// Analisa o texto com os recursos do próprio arquivo e descarta o diagnóstico:
/// aqui só interessa a árvore, transitória do pedido como em [`references`].
fn analyze(text: &str, features: LibraryFeatures) -> File {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    File { names, unit: parsed.unit, ast: parsed.ast }
}

/// Classe de uma diretiva para a resolução entre abertos.
enum DirectiveClass {
    Library,
    /// Import relativo simples: sem prefixo, combinadores, `deferred` ou
    /// condição, com literal constante fora de `dart:`/`package:`.
    SimpleImport(String),
    /// Todo o resto: `export`, `part`, `part of`, augmentations, prefixo,
    /// `show`/`hide`, `dart:`/`package:`, interpolação. Traz nomes que só a
    /// resolução de bibliotecas distingue: arquivo com uma destas nunca é
    /// dono nem testemunha, e cursor sob uma delas é "não sei".
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

/// Resolve um import relativo contra a URI base sem tocar no disco: junta os
/// segmentos (`Url::join`) e devolve a URI normalizada. `None` para
/// `dart:`/`package:`, base fora de `file:`, consulta ou fragmento. Difere do
/// [`resolve`] de propósito: sem `canonicalize`, sem `is_file` — a pergunta
/// aqui é "qual aberto este URI nomeia", nunca "o que existe no disco".
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

/// Resolve o símbolo sob o cursor para o documento aberto que o declara.
///
/// Caso local (sem diretivas além de `library`): o próprio arquivo, como em
/// [`references`]. Caso importado (só imports simples, nome sem declaração
/// local): o único aberto importado que declara o nome sozinho num espaço,
/// ele mesmo sem diretivas que tragam outros nomes. Todo o resto — diretiva
/// complexa, padrões, sombras, parâmetro de tipo homônimo, zero ou vários
/// donos — é `None` ("não sei", nunca resposta errada).
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

/// O único candidato aberto que declara `name` sozinho num espaço, sem
/// diretivas que tragam outros nomes, padrões, sombras ou parâmetro de tipo
/// homônimo. Zero (dono fechado ou no disco) ou vários: `None`.
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
            // Não declara, ou declara nos dois espaços: não é um dono limpo.
            _ => continue,
        };
        if found.is_some() {
            return None;
        }
        found = Some(owner);
    }
    found
}

/// Definição entre documentos abertos: o literal de diretiva segue a regra de
/// disco ([`directive_target`], um `stat`); o nome segue [`resolve_symbol`]
/// (só abertos, nunca o disco). `None` é "não sei".
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

/// Referências entre documentos abertos via [`resolve_symbol`]: a declaração
/// primeiro, depois os usos ordenados por (URI, offset), cada arquivo
/// analisado e liberado antes do próximo — como `workspace/symbol`, nada é
/// retido entre pedidos e o `DocumentStore` continua dono único dos textos.
///
/// Usos de um arquivo só entram quando ele importa o dono por import simples
/// e nada mais pode trazer o nome: diretiva complexa, padrão, sombra,
/// declaração local homônima, import simples para fora dos abertos (cujo
/// conteúdo o pedido não pode ler sem varrer o disco) ou outro aberto que
/// declare o nome — qualquer dúvida pula o arquivo, nunca inventa uso nem
/// devolve lista parcial como total: o chamador não tem como distinguir.
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

/// Usos no documento dono, sem a declaração. `None` quando o dono deixou de
/// ser legível entre a resolução e a coleta (não acontece no despacho
/// síncrono; o `None` conservador evita inventar localização).
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

/// Usos num arquivo que importa o dono por import simples. `None` pula o
/// arquivo (dúvida ou nome ausente por outro motivo); `Some` — mesmo vazio,
/// quando importa mas não usa — contribui.
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
                    // Fora dos abertos: pode declarar o mesmo nome e o pedido
                    // não lê disco por tecla. Pula o arquivo em vez de
                    // atribuir os usos ao dono errado.
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
        // Declaração local homônima: os usos miram ela, não o dono.
        return None;
    }
    Some(uses_without_declaration(&file, key, symbol.space, symbol.declaration))
}

/// Verdadeiro quando o aberto declara `name` em qualquer espaço (tipo ou
/// valor): um importador que também o importa não tem dono único.
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

/// Usos do nome no arquivo, sem a declaração do dono.
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

/// Hover entre documentos abertos: resolve como [`resolve_symbol`] e
/// formata a descrição a partir do documento dono, com as mesmas regras do
/// hover local (tipos não genéricos, variáveis de tipo primitivo escrito,
/// funções de até dois posicionais tipados, getters de retorno primitivo).
/// O `range` devolvido é o da referência sob o cursor, no arquivo vigente.
///
/// Bancada: dono não-aberto, símbolo ambíguo (zero ou dois donos), diretiva
/// complexa (`prefix`, `show`/`hide`, ...), sombra, padrão ou assinatura que
/// exige formatação completa → `None` (hover vazio), nunca texto errado.
/// Nada é retido entre pedidos: cada árvore é transitória, como em
/// [`references_in`]; só os documentos abertos são lidos, nunca o disco.
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

/// Descrição do símbolo a partir do dono, com o mesmo formato do hover
/// local. `None` quando a assinatura não pode ser mostrada com fidelidade
/// (genérico, typedef, tipo não primitivo, parâmetros opcionais/nomeados,
/// retorno inferido, setter): o chamador devolve hover vazio.
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

/// `class C` / `enum E` / `mixin M` / `extension type X`, quando não
/// genérico; mesma regra de [`local_type`].
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

/// Variável de topo com tipo primitivo escrito ou função/getter de topo com
/// assinatura fiel; mesmas regras de [`top_variable`] e [`top_function`].
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
                // Nome de função/getter: não é variável (como em
                // [`top_variable`); cai para a formatação de assinatura
                // abaixo (como em [`top_function`]).
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
