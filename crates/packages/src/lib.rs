//! Carrega o grafo de arquivos Dart com imports relativos do subconjunto.
//!
//! Não resolve símbolos, bibliotecas, privacidade ou namespaces, nem combina a
//! saída de múltiplos arquivos. O lexer existente valida a tokenização inteira.
use dartforge_diagnostics::Span;
use dartforge_syntax::{Token, TokenKind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Grafo com IDs determinísticos pela ordem de descoberta em largura.
#[derive(Debug)]
pub struct SourceGraph {
    /// Arquivos únicos por caminho canônico, indexados pelos IDs dos imports.
    pub units: Vec<SourceUnit>,
    /// ID da entrada solicitada, sempre zero neste carregador.
    pub entry: usize,
}

/// Fonte UTF-8 e diretivas pertencentes a um arquivo.
#[derive(Debug)]
pub struct SourceUnit {
    /// Caminho absoluto canônico do arquivo.
    pub path: PathBuf,
    /// Texto original; os spans dos imports indexam estes bytes.
    pub source: String,
    /// Arestas na ordem textual, inclusive imports repetidos.
    pub imports: Vec<Import>,
}

/// Aresta resolvida de uma diretiva import.
#[derive(Debug)]
pub struct Import {
    /// Caminho relativo escrito na string da diretiva.
    pub uri: String,
    /// Índice da unidade de destino no grafo.
    pub target: usize,
    /// Intervalo da diretiva completa no arquivo importador.
    pub span: Span,
}

/// Erro de leitura, tokenização ou diretiva, associado ao arquivo de origem.
#[derive(Debug)]
pub struct GraphError {
    /// Arquivo ao qual o diagnóstico e o span se referem.
    pub path: PathBuf,
    /// Local da falha; ausente em erros de acesso à entrada.
    pub span: Option<Span>,
    /// Motivo legível, incluindo destino em erros de resolução.
    pub message: String,
}
impl std::fmt::Display for GraphError {
    /// Inclui caminho e, quando disponível, intervalo de bytes da origem.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)?;
        if let Some(span) = self.span {
            write!(f, " (bytes {}..{})", span.start, span.end)?;
        }
        Ok(())
    }
}
impl std::error::Error for GraphError {}

/// Carrega imports relativos, deduplica caminhos canônicos e aceita ciclos.
///
/// As diretivas devem preceder declarações. O carregamento usa uma fila, sem
/// recursão proporcional à profundidade do grafo. Não compila as unidades.
///
/// # Erros
///
/// Retorna erro para arquivos inacessíveis ou não UTF-8, falhas do lexer e
/// diretivas fora do subconjunto. Não aceita package:, dart:, caminhos absolutos,
/// escapes em URI, export, part, library, as, show, hide ou deferred.
///
/// ```no_run
/// use std::path::Path;
/// let graph = dartforge_packages::load(Path::new("main.dart"))?;
/// assert_eq!(graph.entry, 0);
/// println!("{} arquivos", graph.units.len());
/// # Ok::<(), dartforge_packages::GraphError>(())
/// ```
pub fn load(entry: &Path) -> Result<SourceGraph, GraphError> {
    let path =
        std::fs::canonicalize(entry).map_err(|error| error_at(entry, None, error.to_string()))?;
    let source =
        std::fs::read_to_string(&path).map_err(|error| error_at(&path, None, error.to_string()))?;
    let mut known = HashMap::from([(path.clone(), 0)]);
    let mut units = vec![SourceUnit {
        path,
        source,
        imports: vec![],
    }];
    let mut current = 0;
    while current < units.len() {
        let path = units[current].path.clone();
        let directives = extract(&units[current].source, &path)?;
        for (uri, span) in directives {
            let candidate = path
                .parent()
                .expect("arquivo canônico tem diretório pai")
                .join(&uri);
            let target_path = std::fs::canonicalize(&candidate).map_err(|error| {
                error_at(
                    &path,
                    Some(span),
                    format!("não foi possível resolver {}: {error}", candidate.display()),
                )
            })?;
            let target = if let Some(&id) = known.get(&target_path) {
                id
            } else {
                let source = std::fs::read_to_string(&target_path).map_err(|error| {
                    error_at(
                        &path,
                        Some(span),
                        format!("não foi possível ler {}: {error}", target_path.display()),
                    )
                })?;
                let id = units.len();
                known.insert(target_path.clone(), id);
                units.push(SourceUnit {
                    path: target_path,
                    source,
                    imports: vec![],
                });
                id
            };
            units[current].imports.push(Import { uri, target, span });
        }
        current += 1;
    }
    Ok(SourceGraph { units, entry: 0 })
}

/// Constrói diagnóstico sem misturar spans de arquivos diferentes.
fn error_at(path: &Path, span: Option<Span>, message: String) -> GraphError {
    GraphError {
        path: path.to_owned(),
        span,
        message,
    }
}

/// Extrai somente o prefixo de imports, rejeitando diretivas posteriores.
fn extract(source: &str, path: &Path) -> Result<Vec<(String, Span)>, GraphError> {
    let tokens = dartforge_lexer::lex(source)
        .map_err(|error| error_at(path, Some(error.span), error.message))?;
    let mut index = 0;
    let mut imports = Vec::new();
    while tokens
        .get(index)
        .is_some_and(|token| token.kind == TokenKind::Word("import"))
    {
        let start = tokens[index].span.start;
        index += 1;
        let uri =
            match tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::String(uri) | TokenKind::RawString(uri)) if valid_uri(uri) => uri,
                _ => return Err(error_at(
                    path,
                    Some(token_span(&tokens, index, source.len())),
                    "import exige caminho relativo .dart sem escapes, esquema, query ou fragmento"
                        .into(),
                )),
            };
        index += 1;
        if tokens.get(index).map(|token| token.kind) != Some(TokenKind::Symbol(';')) {
            return Err(error_at(path, Some(token_span(&tokens, index, source.len())), "somente import 'relativo.dart'; é suportado; combinadores e imports condicionais não são aceitos".into()));
        }
        imports.push((
            uri.to_owned(),
            Span {
                start,
                end: tokens[index].span.end,
            },
        ));
        index += 1;
    }
    let mut depth = 0usize;
    for token in &tokens[index..] {
        match token.kind {
            TokenKind::Symbol('{') => depth += 1,
            TokenKind::Symbol('}') => depth = depth.saturating_sub(1),
            TokenKind::Word(word @ ("import" | "export" | "part" | "library")) if depth == 0 => {
                return Err(error_at(
                    path,
                    Some(token.span),
                    format!("diretiva {word} fora do prefixo suportado de imports"),
                ));
            }
            _ => {}
        }
    }
    Ok(imports)
}

/// Restringe URIs a caminhos locais portáveis, sem interpretação parcial de URI.
fn valid_uri(uri: &str) -> bool {
    !uri.is_empty()
        && uri.ends_with(".dart")
        && !uri.starts_with('/')
        && !uri
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':' | '%' | '?' | '#'))
        && !Path::new(uri).is_absolute()
}

/// Usa o token corrente ou um span vazio no fim da fonte.
fn token_span(tokens: &[Token<'_>], index: usize, end: usize) -> Span {
    tokens
        .get(index)
        .map_or(Span { start: end, end }, |token| token.span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    /// Diretório exclusivo criado pelos testes e removido ao final.
    struct Fixture(PathBuf);
    impl Fixture {
        /// Reserva um caminho sem reutilizar diretórios preexistentes.
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "dartforge-packages-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(std::fs::canonicalize(path).unwrap()),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("{error}"),
                }
            }
        }
        /// Grava somente arquivos dentro do diretório reservado.
        fn write(&self, path: &str, source: &str) -> PathBuf {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, source).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        /// Limpa o diretório exclusivo; nenhum teste cria links simbólicos.
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    /// IDs seguem largura e ordem textual; diamantes e ciclos reutilizam a unidade.
    #[test]
    fn nested_diamond_cycle_and_alias_are_deterministic() {
        let fixture = Fixture::new();
        let entry = fixture.write(
            "main.dart",
            "// início\nimport 'sub/a.dart'; import r'b.dart'; void main() {}",
        );
        fixture.write("sub/a.dart", "import '../common.dart';");
        fixture.write(
            "b.dart",
            "import './common.dart'; import 'sub/../common.dart';",
        );
        fixture.write("common.dart", "import 'main.dart';");
        let graph = load(&entry).unwrap();
        assert_eq!(graph.entry, 0);
        assert_eq!(graph.units.len(), 4);
        assert_eq!(
            graph
                .units
                .iter()
                .map(|unit| unit.path.file_name().unwrap().to_str().unwrap())
                .collect::<Vec<_>>(),
            ["main.dart", "a.dart", "b.dart", "common.dart"]
        );
        assert_eq!(graph.units[1].imports[0].target, 3);
        assert_eq!(
            graph.units[2]
                .imports
                .iter()
                .map(|import| import.target)
                .collect::<Vec<_>>(),
            [3, 3]
        );
        assert_eq!(graph.units[3].imports[0].target, 0);
        let first = &graph.units[0].imports[0];
        assert_eq!(
            &graph.units[0].source[first.span.start..first.span.end],
            "import 'sub/a.dart';"
        );
        assert_eq!(
            load(&entry)
                .unwrap()
                .units
                .iter()
                .map(|unit| &unit.path)
                .collect::<Vec<_>>(),
            graph
                .units
                .iter()
                .map(|unit| &unit.path)
                .collect::<Vec<_>>()
        );
    }

    /// Falhas de resolução apontam para a diretiva no arquivo importador.
    #[test]
    fn missing_import_reports_owner_and_directive() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'missing.dart';");
        let error = load(&entry).unwrap_err();
        assert_eq!(error.path, std::fs::canonicalize(entry).unwrap());
        assert_eq!(error.span, Some(Span { start: 0, end: 22 }));
        assert!(error.message.contains("missing.dart"));
    }

    /// Não interpreta parcialmente diretivas ou formatos URI fora do contrato.
    #[test]
    fn unsupported_directives_fail_explicitly() {
        let fixture = Fixture::new();
        for source in [
            "import 'package:a/a.dart';",
            "import 'dart:core';",
            "import '/a.dart';",
            "import 'C:/a.dart';",
            "import '../a%20b.dart';",
            "import 'a.dart?q';",
            "import 'a.dart' as a;",
            "import 'a.dart' show A;",
            "import 'a.dart' hide A;",
            "import 'a.dart' deferred as a;",
            "export 'a.dart';",
            "part 'a.dart';",
            "library a;",
            "void main() {} import 'a.dart';",
            "import 'a.dart' if (true) 'b.dart';",
            "import 'a\\x2edart';",
            "import 1;",
            "import 'a.dart'",
        ] {
            let entry = fixture.write("main.dart", source);
            let error = load(&entry).unwrap_err();
            let span = error.span.expect("diretiva deve ter localização");
            assert!(
                span.start <= span.end && span.end <= source.len(),
                "{source}: {error}"
            );
        }
    }

    /// Erros léxicos mantêm o caminho da dependência e seus offsets locais.
    #[test]
    fn dependency_lex_error_keeps_source_path() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'child.dart';");
        let child = fixture.write("child.dart", "/* aberto");
        let error = load(&entry).unwrap_err();
        assert_eq!(error.path, std::fs::canonicalize(child).unwrap());
        assert!(error.span.is_some());
    }

    /// Uma cadeia longa é carregada pela fila sem chamadas recursivas de load.
    #[test]
    fn loads_chain_iteratively() {
        let fixture = Fixture::new();
        for index in 0..128 {
            fixture.write(
                &format!("{index}.dart"),
                &if index == 127 {
                    String::new()
                } else {
                    format!("import '{}.dart';", index + 1)
                },
            );
        }
        assert_eq!(load(&fixture.0.join("0.dart")).unwrap().units.len(), 128);
    }
}
