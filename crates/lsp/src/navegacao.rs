//! Navegação segura de diretivas com URI relativa ou de pacote.
//! O analyzer original registra a região do literal em `ImportDirective`,
//! `ExportDirective` e `PartDirective` quando o destino existe.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{Ast, CompilationUnit, DeclKind, Directive, DirectiveKind, ExprKind, ForInTarget, ForInit, FunctionKind, MemberKind, Name, ParameterKind, StmtKind, StringLit, TypeKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use dartforge_elements::config::PackageConfig;
use url::Url;

use crate::{AnalisadorSintatico, DocumentStore};

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
    if let Some(destino) = alvo_de_diretiva(uri_atual, &parsed.unit, offset) {
        return Some(Alvo::Arquivo(destino));
    }
    tipo_local(&parsed.unit, &parsed.ast, &nomes, offset)
        .or_else(|| variavel_topo(&parsed.unit, &parsed.ast, &nomes, offset))
        .or_else(|| funcao_topo(&parsed.unit, &parsed.ast, &nomes, offset))
        .map(Alvo::NomeLocal)
}

/// Destino de um literal de diretiva (`import`, `export`, `part`, ...) sob o
/// cursor, quando o arquivo relativo existe no disco. É um `stat` por pedido,
/// nunca varredura: a regra de ativação segue a navegação de diretivas do
/// analyzer (só existe alvo quando o arquivo existe).
fn alvo_de_diretiva(uri_atual: &str, unit: &CompilationUnit, offset: usize) -> Option<String> {
    for diretiva in &unit.directives {
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
            return resolver(&base, literal);
        }
    }
    None
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

pub(crate) fn tipo_primitivo(ast: &dartforge_frontend::ast::Ast, nomes: &Interner, id: dartforge_frontend::ast::TypeId) -> Option<String> {
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

/// Referências no próprio documento para os casos seguros de [`destino`].
///
/// Devolve a declaração primeiro e depois os usos em ordem de offset, todos
/// no mesmo arquivo. `None` significa "não sei" (imports, padrões, sombras,
/// nomes duplicados ou cursor fora de nome conhecido) — nunca "não há".
/// Entre arquivos aguarda a resolução de bibliotecas e elementos.
///
/// A árvore é temporária (um `Interner` novo por chamada), como em
/// [`destino`]: nada é retido entre edições, mantendo o platô de memória.
pub(super) fn referencias(
    _uri_atual: &str,
    texto: &str,
    features: LibraryFeatures,
    offset: usize,
) -> Option<Vec<Span>> {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
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
    let chave = chave_sob_cursor(unit, ast, offset)?;
    // Sem sombras em qualquer escopo que o AST enxerga.
    if sombreado(ast, chave) {
        return None;
    }
    if tem_parametro_de_tipo_homonimo(unit, ast, chave) {
        return None;
    }
    let decl_tipo = declaracao_de_tipo(unit, ast, chave);
    let decl_valor = declaracao_de_valor(unit, ast, chave);
    match (decl_tipo, decl_valor) {
        (Some(decl), None) => {
            let mut usos = vec![decl];
            for ty in &ast.types {
                let TypeKind::Named { name, .. } = &ty.kind else { continue };
                if name.len() != 1 || name[0].sym != chave {
                    continue;
                }
                if name[0].span == decl {
                    continue;
                }
                usos.push(name[0].span);
            }
            usos[1..].sort_by_key(|s| s.start);
            let mut saida = vec![usos[0]];
            saida.extend(usos[1..].iter().copied());
            Some(saida)
        }
        (None, Some(decl)) => {
            let mut usos = vec![decl];
            for e in &ast.exprs {
                let ExprKind::Identifier(n) = &e.kind else { continue };
                if n.sym != chave {
                    continue;
                }
                usos.push(n.span);
            }
            usos[1..].sort_by_key(|s| s.start);
            Some(usos)
        }
        // Zero ou dois namespaces com o mesmo nome: conservador, sem resposta.
        _ => None,
    }
}

/// Símbolo sob o cursor, seja numa referência ou no próprio nome declarado.
fn chave_sob_cursor(
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
    if let Some(sym) = nome_de_tipo_declarado_sob_cursor(unit, ast, offset) {
        return Some(sym);
    }
    for e in &ast.exprs {
        let ExprKind::Identifier(n) = &e.kind else { continue };
        if n.span.start <= offset && offset < n.span.end {
            return Some(n.sym);
        }
    }
    nome_de_valor_declarado_sob_cursor(unit, ast, offset)
}

/// Nome de tipo declarado (classe, mixin, enum, extension type, typedef).
fn nome_de_tipo_declarado_sob_cursor(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    offset: usize,
) -> Option<dartforge_intern::SymbolId> {
    for id in &unit.declarations {
        let nome = match &ast.decl(*id).kind {
            DeclKind::Class(d) => Some(d.name),
            DeclKind::Mixin(d) => Some(d.name),
            DeclKind::Enum(d) => Some(d.name),
            DeclKind::ExtensionType(d) => Some(d.name),
            DeclKind::Typedef(d) => Some(d.name),
            _ => None,
        };
        if let Some(n) = nome
            && n.span.start <= offset
            && offset < n.span.end
        {
            return Some(n.sym);
        }
    }
    None
}

/// Nome de valor de topo declarado (variável, função, getter, setter).
fn nome_de_valor_declarado_sob_cursor(
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
fn tem_parametro_de_tipo_homonimo(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    chave: dartforge_intern::SymbolId,
) -> bool {
    if ast.functions.iter().any(|f| f.type_params.iter().any(|p| p.name.sym == chave)) {
        return true;
    }
    if ast.types.iter().any(|t| matches!(&t.kind, TypeKind::Function { type_params, .. }
        if type_params.iter().any(|p| p.name.sym == chave)))
    {
        return true;
    }
    unit.declarations.iter().any(|id| {
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
}

/// Declaração de tipo única com o nome (span do nome declarado).
fn declaracao_de_tipo(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    chave: dartforge_intern::SymbolId,
) -> Option<Span> {
    let mut encontrados = unit.declarations.iter().filter_map(|id| {
        let nome = match &ast.decl(*id).kind {
            DeclKind::Class(d) => Some(d.name),
            DeclKind::Mixin(d) => Some(d.name),
            DeclKind::Enum(d) => Some(d.name),
            DeclKind::ExtensionType(d) => Some(d.name),
            DeclKind::Typedef(d) => Some(d.name),
            _ => None,
        };
        nome.filter(|n| n.sym == chave).map(|n| n.span)
    });
    let unico = encontrados.next()?;
    encontrados.next().is_none().then_some(unico)
}

/// Declaração de valor de topo única com o nome (span do nome declarado).
fn declaracao_de_valor(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    chave: dartforge_intern::SymbolId,
) -> Option<Span> {
    let mut encontrados = Vec::new();
    for id in &unit.declarations {
        match &ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in v.variables.iter().filter(|x| x.name.sym == chave) {
                    encontrados.push(var.name.span);
                }
            }
            DeclKind::Function(f) => {
                if let Some(n) = ast.function(*f).name.filter(|n| n.sym == chave) {
                    encontrados.push(n.span);
                }
            }
            DeclKind::Class(d) if d.name.sym == chave => return None,
            DeclKind::Mixin(d) if d.name.sym == chave => return None,
            DeclKind::Enum(d) if d.name.sym == chave => return None,
            DeclKind::ExtensionType(d) if d.name.sym == chave => return None,
            DeclKind::Typedef(d) if d.name.sym == chave => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == chave) => return None,
            _ => {}
        }
    }
    (encontrados.len() == 1).then(|| encontrados[0])
}

// ---------------------------------------------------------------------------
// Entre documentos abertos: o menor passo útil da resolução semântica.
// ---------------------------------------------------------------------------

/// Espaço de nomes da declaração de topo: tipos e valores não colidem, então
/// cada busca carrega o seu para não misturar `class Caixa` com `int Caixa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Espaco {
    Tipo,
    Valor,
}

/// Símbolo de topo resolvido entre os documentos abertos.
struct Simbolo {
    dono: String,
    espaco: Espaco,
    nome: String,
    declaracao: Span,
}

/// Nome sob o cursor com o seu intervalo: a chave (`SymbolId` só vale no
/// próprio internador) e o span da referência no arquivo vigente. Cobre os
/// mesmos lugares que [`chave_sob_cursor`] (referência ou nome declarado);
/// o hover devolve este span como `range`, no arquivo do cursor.
fn chave_e_span_sob_cursor(
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
        let nome = match &ast.decl(*id).kind {
            DeclKind::Class(d) => Some(d.name),
            DeclKind::Mixin(d) => Some(d.name),
            DeclKind::Enum(d) => Some(d.name),
            DeclKind::ExtensionType(d) => Some(d.name),
            DeclKind::Typedef(d) => Some(d.name),
            _ => None,
        };
        if let Some(n) = nome
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
struct Arquivo {
    nomes: Interner,
    unit: CompilationUnit,
    ast: Ast,
}

/// Analisa o texto com os recursos do próprio arquivo e descarta o diagnóstico:
/// aqui só interessa a árvore, transitória do pedido como em [`referencias`].
fn analisar(texto: &str, features: LibraryFeatures) -> Arquivo {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    Arquivo { nomes, unit: parsed.unit, ast: parsed.ast }
}

/// Classe de uma diretiva para a resolução entre abertos.
enum ClasseDiretiva {
    Biblioteca,
    /// Import relativo simples: sem prefixo, combinadores, `deferred` ou
    /// condição, com literal constante fora de `dart:`/`package:`.
    ImportSimples(String),
    /// Todo o resto: `export`, `part`, `part of`, augmentations, prefixo,
    /// `show`/`hide`, `dart:`/`package:`, interpolação. Traz nomes que só a
    /// resolução de bibliotecas distingue: arquivo com uma destas nunca é
    /// dono nem testemunha, e cursor sob uma delas é "não sei".
    Complexa,
}

fn classificar(diretiva: &Directive) -> ClasseDiretiva {
    match &diretiva.kind {
        DirectiveKind::Library { .. } => ClasseDiretiva::Biblioteca,
        DirectiveKind::Import { uri, configurations, deferred, prefix, combinators } => {
            if *deferred || prefix.is_some() || !combinators.is_empty() || !configurations.is_empty() {
                return ClasseDiretiva::Complexa;
            }
            match uri.constant_value().and_then(|v| v.as_str().map(str::to_string)) {
                Some(caminho)
                    if !caminho.starts_with("dart:")
                        && !caminho.starts_with("package:") =>
                {
                    ClasseDiretiva::ImportSimples(caminho)
                }
                _ => ClasseDiretiva::Complexa,
            }
        }
        _ => ClasseDiretiva::Complexa,
    }
}

/// Resolve um import relativo contra a URI base sem tocar no disco: junta os
/// segmentos (`Url::join`) e devolve a URI normalizada. `None` para
/// `dart:`/`package:`, base fora de `file:`, consulta ou fragmento. Difere do
/// [`resolver`] de propósito: sem `canonicalize`, sem `is_file` — a pergunta
/// aqui é "qual aberto este URI nomeia", nunca "o que existe no disco".
fn resolver_aberto(base_uri: &str, caminho: &str) -> Option<String> {
    if caminho.starts_with("dart:") || caminho.starts_with("package:") {
        return None;
    }
    let base = Url::parse(base_uri).ok()?;
    if base.scheme() != "file" {
        return None;
    }
    let resolvida = base.join(caminho).ok()?;
    if resolvida.scheme() != "file" || resolvida.query().is_some() || resolvida.fragment().is_some() {
        return None;
    }
    Some(resolvida.to_string())
}

/// Resolve o símbolo sob o cursor para o documento aberto que o declara.
///
/// Caso local (sem diretivas além de `library`): o próprio arquivo, como em
/// [`referencias`]. Caso importado (só imports simples, nome sem declaração
/// local): o único aberto importado que declara o nome sozinho num espaço,
/// ele mesmo sem diretivas que tragam outros nomes. Todo o resto — diretiva
/// complexa, padrões, sombras, parâmetro de tipo homônimo, zero ou vários
/// donos — é `None` ("não sei", nunca resposta errada).
fn resolver_simbolo(
    documentos: &DocumentStore,
    uri_atual: &str,
    offset: usize,
    analisador: &mut AnalisadorSintatico,
) -> Option<(Simbolo, Span)> {
    let texto = documentos.get(uri_atual)?.to_string();
    let arquivo = analisar(&texto, analisador.features(uri_atual, &texto));
    if !arquivo.ast.patterns.is_empty() {
        return None;
    }
    let (chave, referencia) = chave_e_span_sob_cursor(&arquivo.unit, &arquivo.ast, offset)?;
    if sombreado(&arquivo.ast, chave)
        || tem_parametro_de_tipo_homonimo(&arquivo.unit, &arquivo.ast, chave)
    {
        return None;
    }
    let nome = arquivo.nomes.resolve(chave).to_string();
    let decl_tipo = declaracao_de_tipo(&arquivo.unit, &arquivo.ast, chave);
    let decl_valor = declaracao_de_valor(&arquivo.unit, &arquivo.ast, chave);
    let mut simples = Vec::new();
    let mut so_simples = true;
    for diretiva in &arquivo.unit.directives {
        match classificar(diretiva) {
            ClasseDiretiva::Biblioteca => {}
            ClasseDiretiva::ImportSimples(caminho) => simples.push(caminho),
            ClasseDiretiva::Complexa => so_simples = false,
        }
    }
    if simples.is_empty() && so_simples {
        return match (decl_tipo, decl_valor) {
            (Some(declaracao), None) => Some((
                Simbolo {
                    dono: uri_atual.to_string(),
                    espaco: Espaco::Tipo,
                    nome,
                    declaracao,
                },
                referencia,
            )),
            (None, Some(declaracao)) => Some((
                Simbolo {
                    dono: uri_atual.to_string(),
                    espaco: Espaco::Valor,
                    nome,
                    declaracao,
                },
                referencia,
            )),
            _ => None,
        };
    }
    if !so_simples || decl_tipo.is_some() || decl_valor.is_some() {
        return None;
    }
    let mut candidatos = Vec::new();
    for caminho in &simples {
        if let Some(uri) = resolver_aberto(uri_atual, caminho) {
            if !candidatos.contains(&uri) {
                candidatos.push(uri);
            }
        }
    }
    dono_unico(documentos, analisador, &candidatos, &nome).map(|s| (s, referencia))
}

/// O único candidato aberto que declara `nome` sozinho num espaço, sem
/// diretivas que tragam outros nomes, padrões, sombras ou parâmetro de tipo
/// homônimo. Zero (dono fechado ou no disco) ou vários: `None`.
fn dono_unico(
    documentos: &DocumentStore,
    analisador: &mut AnalisadorSintatico,
    candidatos: &[String],
    nome: &str,
) -> Option<Simbolo> {
    let mut achado = None;
    for candidato in candidatos {
        let Some(texto) = documentos.get(candidato).map(str::to_string) else {
            continue;
        };
        let arquivo = analisar(&texto, analisador.features(candidato, &texto));
        if !arquivo.ast.patterns.is_empty()
            || arquivo.unit.directives.iter().any(|d| !matches!(&d.kind, DirectiveKind::Library { .. }))
        {
            continue;
        }
        let Some(chave) = arquivo.nomes.lookup(nome) else {
            continue;
        };
        if sombreado(&arquivo.ast, chave)
            || tem_parametro_de_tipo_homonimo(&arquivo.unit, &arquivo.ast, chave)
        {
            continue;
        }
        let dono = match (
            declaracao_de_tipo(&arquivo.unit, &arquivo.ast, chave),
            declaracao_de_valor(&arquivo.unit, &arquivo.ast, chave),
        ) {
            (Some(declaracao), None) => Simbolo {
                dono: candidato.clone(),
                espaco: Espaco::Tipo,
                nome: nome.to_string(),
                declaracao,
            },
            (None, Some(declaracao)) => Simbolo {
                dono: candidato.clone(),
                espaco: Espaco::Valor,
                nome: nome.to_string(),
                declaracao,
            },
            // Não declara, ou declara nos dois espaços: não é um dono limpo.
            _ => continue,
        };
        if achado.is_some() {
            return None;
        }
        achado = Some(dono);
    }
    achado
}

/// Definição entre documentos abertos: o literal de diretiva segue a regra de
/// disco ([`alvo_de_diretiva`], um `stat`); o nome segue [`resolver_simbolo`]
/// (só abertos, nunca o disco). `None` é "não sei".
pub(super) fn definicao_em(
    documentos: &DocumentStore,
    uri_atual: &str,
    offset: usize,
    analisador: &mut AnalisadorSintatico,
) -> Option<(String, Option<Span>)> {
    let texto = documentos.get(uri_atual)?.to_string();
    let features = analisador.features(uri_atual, &texto);
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(&texto, &mut nomes, features);
    if let Some(destino) = alvo_de_diretiva(uri_atual, &parsed.unit, offset) {
        return Some((destino, None));
    }
    let simbolo = resolver_simbolo(documentos, uri_atual, offset, analisador)?.0;
    Some((simbolo.dono, Some(simbolo.declaracao)))
}

/// Referências entre documentos abertos via [`resolver_simbolo`]: a declaração
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
pub(super) fn referencias_em(
    documentos: &DocumentStore,
    uri_atual: &str,
    offset: usize,
    analisador: &mut AnalisadorSintatico,
) -> Option<Vec<(String, Span)>> {
    let simbolo = resolver_simbolo(documentos, uri_atual, offset, analisador)?.0;
    let mut saidas = vec![(simbolo.dono.clone(), simbolo.declaracao)];
    let mut uris: Vec<String> = documentos.uris().map(str::to_string).collect();
    uris.sort_unstable();
    for uri in uris {
        let usos = if uri == simbolo.dono {
            usos_do_dono(documentos, analisador, &simbolo)?
        } else {
            let Some(usos) = usos_se_importa(documentos, analisador, &uri, &simbolo) else {
                continue;
            };
            usos
        };
        saidas.extend(usos.into_iter().map(|span| (uri.clone(), span)));
    }    saidas[1..].sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.start.cmp(&b.1.start)));
    Some(saidas)
}

/// Usos no documento dono, sem a declaração. `None` quando o dono deixou de
/// ser legível entre a resolução e a coleta (não acontece no despacho
/// síncrono; o `None` conservador evita inventar localização).
fn usos_do_dono(
    documentos: &DocumentStore,
    analisador: &mut AnalisadorSintatico,
    simbolo: &Simbolo,
) -> Option<Vec<Span>> {
    let texto = documentos.get(&simbolo.dono)?.to_string();
    let arquivo = analisar(&texto, analisador.features(&simbolo.dono, &texto));
    let chave = arquivo.nomes.lookup(&simbolo.nome)?;
    Some(usos_sem_declaracao(&arquivo, chave, simbolo.espaco, simbolo.declaracao))
}

/// Usos num arquivo que importa o dono por import simples. `None` pula o
/// arquivo (dúvida ou nome ausente por outro motivo); `Some` — mesmo vazio,
/// quando importa mas não usa — contribui.
fn usos_se_importa(
    documentos: &DocumentStore,
    analisador: &mut AnalisadorSintatico,
    uri: &str,
    simbolo: &Simbolo,
) -> Option<Vec<Span>> {
    let texto = documentos.get(uri)?.to_string();
    let arquivo = analisar(&texto, analisador.features(uri, &texto));
    if !arquivo.ast.patterns.is_empty() {
        return None;
    }
    let mut importa_dono = false;
    for diretiva in &arquivo.unit.directives {
        match classificar(diretiva) {
            ClasseDiretiva::Biblioteca => {}
            ClasseDiretiva::ImportSimples(caminho) => {
                let Some(alvo) = resolver_aberto(uri, &caminho) else {
                    return None;
                };
                if alvo == simbolo.dono {
                    importa_dono = true;
                } else if documentos.get(&alvo).is_some() {
                    if declara_nome(documentos, analisador, &alvo, &simbolo.nome) {
                        return None;
                    }
                } else {
                    // Fora dos abertos: pode declarar o mesmo nome e o pedido
                    // não lê disco por tecla. Pula o arquivo em vez de
                    // atribuir os usos ao dono errado.
                    return None;
                }
            }
            ClasseDiretiva::Complexa => return None,
        }
    }
    if !importa_dono {
        return None;
    }
    let Some(chave) = arquivo.nomes.lookup(&simbolo.nome) else {
        return Some(Vec::new());
    };
    if sombreado(&arquivo.ast, chave) {
        return None;
    }
    if simbolo.espaco == Espaco::Tipo
        && tem_parametro_de_tipo_homonimo(&arquivo.unit, &arquivo.ast, chave)
    {
        return None;
    }
    if declaracao_de_tipo(&arquivo.unit, &arquivo.ast, chave).is_some()
        || declaracao_de_valor(&arquivo.unit, &arquivo.ast, chave).is_some()
    {
        // Declaração local homônima: os usos miram ela, não o dono.
        return None;
    }
    Some(usos_sem_declaracao(&arquivo, chave, simbolo.espaco, simbolo.declaracao))
}

/// Verdadeiro quando o aberto declara `nome` em qualquer espaço (tipo ou
/// valor): um importador que também o importa não tem dono único.
fn declara_nome(
    documentos: &DocumentStore,
    analisador: &mut AnalisadorSintatico,
    uri: &str,
    nome: &str,
) -> bool {
    let Some(texto) = documentos.get(uri).map(str::to_string) else {
        return false;
    };
    let arquivo = analisar(&texto, analisador.features(uri, &texto));
    let Some(chave) = arquivo.nomes.lookup(nome) else {
        return false;
    };
    declaracao_de_tipo(&arquivo.unit, &arquivo.ast, chave).is_some()
        || declaracao_de_valor(&arquivo.unit, &arquivo.ast, chave).is_some()
}

/// Usos do nome no arquivo, sem a declaração do dono.
fn usos_sem_declaracao(
    arquivo: &Arquivo,
    chave: dartforge_intern::SymbolId,
    espaco: Espaco,
    declaracao: Span,
) -> Vec<Span> {
    let mut usos = Vec::new();
    match espaco {
        Espaco::Tipo => {
            for ty in &arquivo.ast.types {
                let TypeKind::Named { name, .. } = &ty.kind else { continue };
                if name.len() != 1 || name[0].sym != chave || name[0].span == declaracao {
                    continue;
                }
                usos.push(name[0].span);
            }
        }
        Espaco::Valor => {
            for e in &arquivo.ast.exprs {
                let ExprKind::Identifier(n) = &e.kind else { continue };
                if n.sym != chave {
                    continue;
                }
                usos.push(n.span);
            }
        }
    }
    usos.sort_by_key(|s| s.start);
    usos
}

/// Hover entre documentos abertos: resolve como [`resolver_simbolo`] e
/// formata a descrição a partir do documento dono, com as mesmas regras do
/// hover local (tipos não genéricos, variáveis de tipo primitivo escrito,
/// funções de até dois posicionais tipados, getters de retorno primitivo).
/// O `range` devolvido é o da referência sob o cursor, no arquivo vigente.
///
/// Bancada: dono não-aberto, símbolo ambíguo (zero ou dois donos), diretiva
/// complexa (`prefixo`, `show`/`hide`, ...), sombra, padrão ou assinatura que
/// exige formatação completa → `None` (hover vazio), nunca texto errado.
/// Nada é retido entre pedidos: cada árvore é transitória, como em
/// [`referencias_em`]; só os documentos abertos são lidos, nunca o disco.
pub(super) fn hover_em(
    documentos: &DocumentStore,
    uri_atual: &str,
    offset: usize,
    analisador: &mut AnalisadorSintatico,
) -> Option<(Span, String, Option<String>)> {
    let (simbolo, referencia) = resolver_simbolo(documentos, uri_atual, offset, analisador)?;
    let (descricao, tipo) = descricao_no_dono(documentos, analisador, &simbolo)?;
    Some((referencia, descricao, tipo))
}

/// Descrição do símbolo a partir do dono, com o mesmo formato do hover
/// local. `None` quando a assinatura não pode ser mostrada com fidelidade
/// (genérico, typedef, tipo não primitivo, parâmetros opcionais/nomeados,
/// retorno inferido, setter): o chamador devolve hover vazio.
fn descricao_no_dono(
    documentos: &DocumentStore,
    analisador: &mut AnalisadorSintatico,
    simbolo: &Simbolo,
) -> Option<(String, Option<String>)> {
    let texto = documentos.get(&simbolo.dono)?.to_string();
    let arquivo = analisar(&texto, analisador.features(&simbolo.dono, &texto));
    let chave = arquivo.nomes.lookup(&simbolo.nome)?;
    match simbolo.espaco {
        Espaco::Tipo => descricao_tipo_no_arquivo(&arquivo, chave).map(|d| (d, None)),
        Espaco::Valor => descricao_valor_no_arquivo(&arquivo, chave),
    }
}

/// `class C` / `enum E` / `mixin M` / `extension type X`, quando não
/// genérico; mesma regra de [`tipo_local`].
fn descricao_tipo_no_arquivo(
    arquivo: &Arquivo,
    chave: dartforge_intern::SymbolId,
) -> Option<String> {
    let mut encontrados = arquivo.unit.declarations.iter().filter_map(|id| {
        let (nome, tipo, generico) = match &arquivo.ast.decl(*id).kind {
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
    if encontrados.next().is_some() {
        return None;
    }
    (!unico.2).then(|| format!("{} {}", unico.1, arquivo.nomes.resolve(chave)))
}

/// Variável de topo com tipo primitivo escrito ou função/getter de topo com
/// assinatura fiel; mesmas regras de [`variavel_topo`] e [`funcao_topo`].
fn descricao_valor_no_arquivo(
    arquivo: &Arquivo,
    chave: dartforge_intern::SymbolId,
) -> Option<(String, Option<String>)> {
    let mut variaveis = Vec::new();
    let mut tem_funcao = false;
    for id in &arquivo.unit.declarations {
        match &arquivo.ast.decl(*id).kind {
            DeclKind::Variables(v) => {
                for var in v.variables.iter().filter(|x| x.name.sym == chave) {
                    variaveis.push((var.name.span, v.ty));
                }
            }
            DeclKind::Class(d) if d.name.sym == chave => return None,
            DeclKind::Mixin(d) if d.name.sym == chave => return None,
            DeclKind::Enum(d) if d.name.sym == chave => return None,
            DeclKind::ExtensionType(d) if d.name.sym == chave => return None,
            DeclKind::Typedef(d) if d.name.sym == chave => return None,
            DeclKind::Function(f)
                if arquivo.ast.function(*f).name.is_some_and(|n| n.sym == chave) =>
            {
                // Nome de função/getter: não é variável (como em
                // [`variavel_topo`); cai para a formatação de assinatura
                // abaixo (como em [`funcao_topo`]).
                tem_funcao = true;
            }
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == chave) => return None,
            _ => {}
        }
    }
    if !variaveis.is_empty() {
        if tem_funcao || variaveis.len() != 1 {
            return None;
        }
        let tipo = tipo_primitivo(&arquivo.ast, &arquivo.nomes, variaveis[0].1?)?;
        let descricao = format!("{tipo} {}", arquivo.nomes.resolve(chave));
        return Some((descricao, Some(tipo)));
    }
    if !tem_funcao {
        return None;
    }
    let mut funcoes = Vec::new();
    for id in &arquivo.unit.declarations {
        match &arquivo.ast.decl(*id).kind {
            DeclKind::Function(f) => {
                let f = arquivo.ast.function(*f);
                if let Some(nome) = f.name.filter(|n| n.sym == chave) {
                    funcoes.push((nome.span, f));
                }
            }
            DeclKind::Variables(v)
                if v.variables.iter().any(|x| x.name.sym == chave) =>
            {
                return None;
            }
            DeclKind::Class(d) if d.name.sym == chave => return None,
            DeclKind::Mixin(d) if d.name.sym == chave => return None,
            DeclKind::Enum(d) if d.name.sym == chave => return None,
            DeclKind::ExtensionType(d) if d.name.sym == chave => return None,
            DeclKind::Typedef(d) if d.name.sym == chave => return None,
            DeclKind::Extension(d) if d.name.is_some_and(|n| n.sym == chave) => return None,
            _ => {}
        }
    }
    if funcoes.len() != 1 {
        return None;
    }
    let funcao = funcoes[0].1;
    let (descricao, tipo) = (|| {
        if !funcao.type_params.is_empty() {
            return None;
        }
        let retorno = funcao.return_type.and_then(|t| {
            if matches!(&arquivo.ast.ty(t).kind, TypeKind::Void) {
                Some("void".to_string())
            } else {
                tipo_primitivo(&arquivo.ast, &arquivo.nomes, t)
            }
        })?;
        if funcao.kind == FunctionKind::Getter {
            return funcao
                .parameters
                .is_none()
                .then(|| (format!("{retorno} get {}", arquivo.nomes.resolve(chave)), Some(retorno)));
        }
        if funcao.kind != FunctionKind::Function {
            return None;
        }
        let parametros = funcao.parameters.as_ref()?;
        if parametros.len() > 2 {
            return None;
        }
        let mut descricoes = Vec::new();
        for p in parametros.iter() {
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
            let tipo = tipo_primitivo(&arquivo.ast, &arquivo.nomes, p.ty?)?;
            let nome = arquivo.nomes.resolve(p.name?.sym);
            descricoes.push(format!("{tipo} {nome}"));
        }
        Some((
            format!(
                "{retorno} {}({})",
                arquivo.nomes.resolve(chave),
                descricoes.join(", ")
            ),
            None,
        ))
    })()?;
    Some((descricao, tipo))
}
