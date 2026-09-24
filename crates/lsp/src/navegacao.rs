//! Navegação segura de diretivas com URI relativa.
//! O analyzer original registra a região do literal em `ImportDirective`,
//! `ExportDirective` e `PartDirective` quando o destino existe.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{DeclKind, DirectiveKind, ExprKind, ForInTarget, ForInit, FunctionKind, MemberKind, Name, ParameterKind, StmtKind, StringLit, TypeKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
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

/// Destino para um literal de URI relativa ou tipo único do próprio arquivo.
/// Bibliotecas `dart:` e `package:` exigem resolução de SDK/pacotes e ficam
/// para a costura semântica; caminho inexistente não gera navegação falsa.
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

fn referencia_expr(ast: &dartforge_frontend::ast::Ast, offset: usize) -> Option<Name> {
    ast.exprs.iter().find_map(|e| {
        let ExprKind::Identifier(n) = &e.kind else { return None };
        (n.span.start <= offset && offset < n.span.end).then_some(*n)
    })
}

fn sombreado(ast: &dartforge_frontend::ast::Ast, chave: dartforge_intern::SymbolId) -> bool {
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
    // `Url::join` resolveria `package:` como outra scheme; nunca anunciar uma
    // localização com semântica diferente da resolução Dart do projeto.
    if caminho.starts_with("dart:") || caminho.starts_with("package:") {
        return None;
    }
    let resolvida = base.join(caminho).ok()?;
    if resolvida.scheme() != "file" || resolvida.query().is_some() || resolvida.fragment().is_some() {
        return None;
    }
    let destino = resolvida.to_file_path().ok()?;
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
