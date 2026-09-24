//! Navegação segura de diretivas com URI relativa.
//! O analyzer original registra a região do literal em `ImportDirective`,
//! `ExportDirective` e `PartDirective` quando o destino existe.

use dartforge_frontend::ast::{DirectiveKind, StringLit};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use url::Url;

/// URI `file:` de destino para o literal sob `offset` (bytes UTF-8).
/// Bibliotecas `dart:` e `package:` exigem resolução de SDK/pacotes e ficam
/// para a costura semântica; caminho inexistente não gera navegação falsa.
pub(super) fn uri_relativa(
    uri_atual: &str,
    texto: &str,
    features: LibraryFeatures,
    offset: usize,
) -> Option<String> {
    let mut nomes = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    let base = Url::parse(uri_atual).ok()?;
    if base.scheme() != "file" {
        return None;
    }
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
            return resolver(&base, literal);
        }
    }
    None
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
