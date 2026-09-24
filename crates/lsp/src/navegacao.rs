//! Navegação segura de diretivas com URI relativa ou de pacote.
//! O analyzer original registra a região do literal em `ImportDirective`,
//! `ExportDirective` e `PartDirective` quando o destino existe.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{DeclKind, DirectiveKind, ExprKind, ForInTarget, ForInit, FunctionKind, MemberKind, Name, ParameterKind, StmtKind, StringLit, TypeKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use dartforge_elements::config::PackageConfig;
use url::Url;

pub(super) enum Alvo {
    Arquivo(String),
    NomeLocal(TipoLocal),
}

pub(super) struct TipoLocal {
    pub declaracao: Span,
    pub referencia: Span,
    pub descricao: Option<String>,
    pub tipo_estatico: Option<String>,
}

/// Destino para um literal de URI relativa/de pacote ou tipo único do próprio
/// arquivo. Bibliotecas `dart:` exigem o mapeamento do SDK; caminho inexistente
/// não gera navegação falsa.
pub(super) fn destino(
    uri_atual: &str,
    texto: &str,
    features: LibraryFeatures,
    offset: usize,
) -> Option<Alvo> {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    for diretiva in &parsed.unit.directives {
        let literal = match &diretiva.kind {
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
            let base = Url::parse(uri_atual).ok()?;
            if base.scheme() != "file" { return None; }
            return resolver(&base, literal).map(Alvo::Arquivo);
        }
    }
    tipo_local(&parsed.unit, &parsed.ast, &nomes, offset)
        .or_else(|| variavel_topo(&parsed.unit, &parsed.ast, &nomes, offset))
        .or_else(|| funcao_topo(&parsed.unit, &parsed.ast, &nomes, offset))
        .map(Alvo::NomeLocal)
}

fn tipo_local(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    nomes: &Interner,
    offset: usize,
) -> Option<TipoLocal> {
    // Imports/exports/parts podem trazer nomes que o AST de um arquivo só não
    // distingue. Até a resolução de biblioteca entrar no LSP, devolva vazio.
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. })) {
        return None;
    }
    let referencia = ast.types.iter().find_map(|ty| {
        let TypeKind::Named { name, .. } = &ty.kind else { return None };
        if name.len() != 1 { return None; }
        let n = name[0];
        (n.span.start <= offset && offset < n.span.end).then_some(n)
    })?;
    let chave = referencia.sym;
    // Um parâmetro de tipo homônimo pode sombrear a declaração de topo.
    if ast.functions.iter().any(|f| f.type_params.iter().any(|p| p.name.sym == chave))
        || ast.types.iter().any(|t| matches!(&t.kind, TypeKind::Function { type_params, .. }
            if type_params.iter().any(|p| p.name.sym == chave)))
        || unit.declarations.iter().any(|id| {
            let parametros = match &ast.decl(*id).kind {
                DeclKind::Class(d) => d.type_params.as_ref(),
                DeclKind::Mixin(d) => d.type_params.as_ref(),
                DeclKind::Enum(d) => d.type_params.as_ref(),
                DeclKind::Extension(d) => d.type_params.as_ref(),
                DeclKind::ExtensionType(d) => d.type_params.as_ref(),
                DeclKind::Typedef(d) => d.type_params.as_ref(),
                _ => &[],
            };
            parametros.iter().any(|p| p.name.sym == chave)
        })
    {
        return None;
    }
    let mut encontrados = unit.declarations.iter().filter_map(|id| {
        let (nome, tipo, generico) = match &ast.decl(*id).kind {
            DeclKind::Class(d) => (d.name, "class", !d.type_params.is_empty()),
            DeclKind::Mixin(d) => (d.name, "mixin", !d.type_params.is_empty()),
            DeclKind::Enum(d) => (d.name, "enum", !d.type_params.is_empty()),
            DeclKind::ExtensionType(d) => (d.name, "extension type", !d.type_params.is_empty()),
            DeclKind::Typedef(d) => (d.name, "typedef", true),
            _ => return None,
        };
        (nome.sym == chave).then_some((nome.span, tipo, generico))
    });
    let unico = encontrados.next()?;
    if encontrados.next().is_some() { return None; }
    Some(TipoLocal {
        declaracao: unico.0,
        referencia: referencia.span,
        descricao: (!unico.2).then(|| format!("{} {}", unico.1, nomes.resolve(chave))),
        tipo_estatico: None,
    })
}

/// Referência a variável de topo única, sem qualquer declaração homônima que
/// possa sombreá-la. Só os tipos primitivos escritos dão hover: os outros
/// precisam da resolução de aliases, limites e imports.
fn variavel_topo(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    nomes: &Interner,
    offset: usize,
) -> Option<TipoLocal> {
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        || !ast.patterns.is_empty()
    {
        return None;
    }
    let referencia = referencia_expr(ast, offset)?;
    let chave = referencia.sym;
    if sombreado(ast, chave) { return None; }
    let mut encontrados = Vec::new();
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in v.variables.iter().filter(|x| x.name.sym == chave) {
                    encontrados.push((var.name.span, v.ty));
                }
            }
            DeclKind::Class(d) if d.name.sym == chave => return None,
            DeclKind::Mixin(d) if d.name.sym == chave => return None,
            DeclKind::Enum(d) if d.name.sym == chave => return None,
            DeclKind::ExtensionType(d) if d.name.sym == chave => return None,
            DeclKind::Typedef(d) if d.name.sym == chave => return None,
            DeclKind::Function(f) if ast.function(*f).name.is_some_and(|n| n.sym == chave) => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == chave) => return None,
            _ => {}
        }
    }
    if encontrados.len() != 1 { return None; }
    let (declaracao, ty) = encontrados[0];
    let tipo_estatico = ty.and_then(|t| tipo_primitivo(ast, nomes, t));
    let descricao = tipo_estatico.as_ref().map(|t| format!("{t} {}", nomes.resolve(chave)));
    Some(TipoLocal { declaracao, referencia: referencia.span, descricao, tipo_estatico })
}

pub(super) fn referencia_expr(ast: &dartforge_frontend::ast::Ast, offset: usize) -> Option<Name> {
    ast.exprs.iter().find_map(|e| {
        let ExprKind::Identifier(n) = &e.kind else { return None };
        (n.span.start <= offset && offset < n.span.end).then_some(*n)
    })
}

pub(super) fn sombreado(ast: &dartforge_frontend::ast::Ast, chave: dartforge_intern::SymbolId) -> bool {
    ast.functions.iter().any(|f| f.parameters.as_ref().is_some_and(|ps| ps.iter().any(|p| p.name.is_some_and(|n| n.sym == chave))))
        || ast.members.iter().any(|m| match &m.kind {
            MemberKind::Field(v) => v.variables.iter().any(|x| x.name.sym == chave),
            MemberKind::Method(f) => ast.function(*f).name.is_some_and(|n| n.sym == chave),
            MemberKind::Constructor(c) => c.name.is_some_and(|n| n.sym == chave),
        })
        || ast.stmts.iter().any(|s| match &s.kind {
            StmtKind::Variables(v) => v.variables.iter().any(|x| x.name.sym == chave),
            StmtKind::Function(f) => ast.function(*f).name.is_some_and(|n| n.sym == chave),
            StmtKind::For { init: Some(ForInit::Variables(v)), .. } =>
                v.variables.iter().any(|x| x.name.sym == chave),
            StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => name.sym == chave,
            StmtKind::Try { catches, .. } => catches.iter().any(|c|
                c.exception.is_some_and(|n| n.sym == chave)
                    || c.stack_trace.is_some_and(|n| n.sym == chave)),
            _ => false,
        })
}

fn tipo_primitivo(ast: &dartforge_frontend::ast::Ast, nomes: &Interner, id: dartforge_frontend::ast::TypeId) -> Option<String> {
    let t = ast.ty(id);
    let TypeKind::Named { name, args } = &t.kind else { return None };
    if name.len() != 1 || !args.is_empty() { return None; }
    let base = nomes.resolve(name[0].sym);
    matches!(base, "int" | "double" | "num" | "bool" | "String" | "Object" | "dynamic")
        .then(|| format!("{base}{}", if t.nullable { "?" } else { "" }))
}

fn funcao_topo(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    nomes: &Interner,
    offset: usize,
) -> Option<TipoLocal> {
    if unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        || !ast.patterns.is_empty()
    { return None; }
    let referencia = referencia_expr(ast, offset)?;
    let chave = referencia.sym;
    if sombreado(ast, chave) { return None; }
    let mut encontrados = Vec::new();
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Function(f) => {
                let f = ast.function(*f);
                if let Some(nome) = f.name.filter(|n| n.sym == chave) {
                    encontrados.push((nome.span, f));
                }
            }
            DeclKind::Variables(v) if v.variables.iter().any(|x| x.name.sym == chave) => return None,
            DeclKind::Class(d) if d.name.sym == chave => return None,
            DeclKind::Mixin(d) if d.name.sym == chave => return None,
            DeclKind::Enum(d) if d.name.sym == chave => return None,
            DeclKind::ExtensionType(d) if d.name.sym == chave => return None,
            DeclKind::Typedef(d) if d.name.sym == chave => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == chave) => return None,
            _ => {}
        }
    }
    if encontrados.len() != 1 { return None; }
    let (declaracao, funcao) = encontrados[0];
    let descricao = (|| {
        if !funcao.type_params.is_empty() {
            return None;
        }
        let retorno = funcao.return_type.and_then(|t| {
            if matches!(&ast.ty(t).kind, TypeKind::Void) {
                Some("void".to_string())
            } else {
                tipo_primitivo(ast, nomes, t)
            }
        })?;
        if funcao.kind == FunctionKind::Getter {
            return funcao.parameters.is_none().then(|| (format!("{retorno} get {}", nomes.resolve(chave)), Some(retorno)));
        }
        if funcao.kind != FunctionKind::Function { return None; }
        let parametros = funcao.parameters.as_ref()?;
        // Com 3+ parâmetros, o analyzer muda para layout multilinha.
        if parametros.len() > 2 { return None; }
        let mut descricoes = Vec::new();
        for p in parametros.iter() {
            if p.kind != ParameterKind::Required || p.covariant || p.final_ || p.var_ || p.const_
                || p.this_ || p.super_ || p.default_value.is_some()
                || !p.function_type_params.is_empty() || p.function_parameters.is_some()
            { return None; }
            let tipo = tipo_primitivo(ast, nomes, p.ty?)?;
            let nome = nomes.resolve(p.name?.sym);
            descricoes.push(format!("{tipo} {nome}"));
        }
        Some((format!("{retorno} {}({})", nomes.resolve(chave), descricoes.join(", ")), None))
    })();
    let (descricao, tipo_estatico) = descricao.map_or((None, None), |(d, t)| (Some(d), t));
    Some(TipoLocal { declaracao, referencia: referencia.span, descricao, tipo_estatico })
}

fn resolver(base: &Url, literal: &StringLit) -> Option<String> {
    let valor = literal.constant_value()?;
    let caminho = valor.as_str()?;
    if caminho.starts_with("dart:") {
        return None;
    }
    let destino = if caminho.starts_with("package:") {
        let uri = Url::parse(caminho).ok()?;
        if uri.query().is_some() || uri.fragment().is_some() { return None; }
        let origem = base.to_file_path().ok()?;
        let config = PackageConfig::discover(&origem)?;
        let config = PackageConfig::load(&config).ok()?;
        let nome = caminho.strip_prefix("package:")?.split('/').next()?;
        // O resolver de compilação tem fallback para `references/pub`; no LSP
        // a navegação deve seguir apenas o mapeamento do projeto aberto.
        if !config.packages.contains_key(nome) { return None; }
        config.resolve_package_uri(caminho).ok()?
    } else {
        let resolvida = base.join(caminho).ok()?;
        if resolvida.scheme() != "file" || resolvida.query().is_some() || resolvida.fragment().is_some() {
            return None;
        }
        resolvida.to_file_path().ok()?
    };
    if !destino.is_file() {
        return None;
    }
    let canonico = std::fs::canonicalize(destino).ok()?;
    Url::from_file_path(canonico).ok().map(|u| u.to_string())
}
