//! Navegação segura de diretivas com URI relativa.
//! O analyzer original registra a região do literal em `ImportDirective`,
//! `ExportDirective` e `PartDirective` quando o destino existe.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{DeclKind, DirectiveKind, StringLit, TypeKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use url::Url;

pub(super) enum Alvo {
    Arquivo(String),
    NomeLocal(Span),
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
    tipo_local(&parsed.unit, &parsed.ast, offset).map(Alvo::NomeLocal)
}

fn tipo_local(
    unit: &dartforge_frontend::ast::CompilationUnit,
    ast: &dartforge_frontend::ast::Ast,
    offset: usize,
) -> Option<Span> {
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
        let nome = match &ast.decl(*id).kind {
            DeclKind::Class(d) => d.name,
            DeclKind::Mixin(d) => d.name,
            DeclKind::Enum(d) => d.name,
            DeclKind::ExtensionType(d) => d.name,
            DeclKind::Typedef(d) => d.name,
            _ => return None,
        };
        (nome.sym == chave).then_some(nome.span)
    });
    let unico = encontrados.next()?;
    encontrados.next().is_none().then_some(unico)
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
