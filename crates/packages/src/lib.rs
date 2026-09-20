//! Carrega arquivos Dart, imports/exports locais e package_config v2 do subconjunto.
//!
//! Não resolve símbolos, bibliotecas, privacidade ou namespaces, nem combina a
//! saída de múltiplos arquivos. O lexer existente valida a tokenização inteira.
//! Condicionais selecionam a primeira alternativa verdadeira; destinos inativos
//! não são resolvidos nem lidos. A análise do prefixo independe do perfil alvo.
mod config;
mod environment;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{Token, TokenKind};
pub use environment::{CompilationEnvironment, CompilationTarget};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Grafo com IDs determinísticos pela ordem de descoberta em largura.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceGraph {
    /// Perfil e definições usados para selecionar as arestas condicionais.
    pub environment: CompilationEnvironment,
    /// Arquivos únicos por caminho canônico, indexados pelos IDs dos imports.
    pub units: Vec<SourceUnit>,
    /// ID da entrada solicitada, sempre zero neste carregador.
    pub entry: usize,
}

/// Fonte UTF-8 e diretivas pertencentes a um arquivo.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceUnit {
    /// Caminho absoluto canônico do arquivo.
    pub path: PathBuf,
    /// Texto original; os spans dos imports indexam estes bytes.
    pub source: String,
    /// Arestas na ordem textual, inclusive imports repetidos.
    pub imports: Vec<Import>,
    /// Arestas exportadas na ordem textual, com filtros show/hide.
    pub exports: Vec<Import>,
}

/// Aresta resolvida de uma diretiva import.
#[derive(Debug, PartialEq, Eq)]
pub struct Import {
    /// Caminho relativo escrito na string da diretiva.
    pub uri: String,
    /// Filtros aplicados sequencialmente ao namespace importado ou exportado.
    pub combinators: Vec<Combinator>,
    /// Índice da unidade de destino no grafo.
    pub target: usize,
    /// Intervalo da diretiva completa no arquivo importador.
    pub span: Span,
}

/// Filtro de nomes aplicado na ordem das cláusulas da diretiva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Combinator {
    Show(Vec<String>),
    Hide(Vec<String>),
}
/// Erro de leitura, tokenização ou diretiva, associado ao arquivo de origem.
#[derive(Debug, PartialEq, Eq)]
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

/// Carrega imports/exports relativos ou package:, deduplicando caminhos e aceitando ciclos.
///
/// As diretivas devem preceder declarações. O carregamento usa uma fila, sem
/// recursão proporcional à profundidade do grafo. Não compila as unidades.
///
/// # Erros
///
/// Retorna erro para arquivos inacessíveis ou não UTF-8, falhas do lexer e
/// diretivas fora do subconjunto. Aceita show/hide sequenciais e package_config v2.
/// Aceita dart:core sem filtros; outras bibliotecas dart:, part, library e aliases são rejeitados.
///
/// ```no_run
/// use std::path::Path;
/// let graph = dartforge_packages::load(Path::new("main.dart"))?;
/// assert_eq!(graph.entry, 0);
/// println!("{} arquivos", graph.units.len());
/// # Ok::<(), dartforge_packages::GraphError>(())
/// ```
pub fn load(entry: &Path) -> Result<SourceGraph, GraphError> {
    load_with_config(entry, None)
}

/// Carrega o grafo usando configuração explícita ou descoberta ascendente.
///
/// # Erros
/// Retorna erros localizados de configuração, URI, diretiva ou leitura de fonte.
///
/// Exemplo de configuração explícita sem alterar a descoberta padrão.
///
///     let graph = dartforge_packages::load_with_config(std::path::Path::new("main.dart"), Some(std::path::Path::new("package_config.json")));
///     // O resultado contém o grafo ou o diagnóstico localizado.
pub fn load_with_config(
    entry: &Path,
    package_config: Option<&Path>,
) -> Result<SourceGraph, GraphError> {
    load_with_config_and_environment(entry, package_config, &CompilationEnvironment::javascript())
}

/// Carrega apenas as alternativas selecionadas pelo ambiente de compilação.
///
/// # Erros
/// Retorna erros de sintaxe de todas as diretivas ou de resolução dos destinos ativos.
///
/// ```no_run
/// let ambiente = dartforge_packages::CompilationEnvironment::native();
/// let grafo = dartforge_packages::load_with_environment(std::path::Path::new("main.dart"), &ambiente)?;
/// # Ok::<(), dartforge_packages::GraphError>(())
/// ```
pub fn load_with_environment(
    entry: &Path,
    environment: &CompilationEnvironment,
) -> Result<SourceGraph, GraphError> {
    load_with_config_and_environment(entry, None, environment)
}

/// Combina configuração de pacotes explícita ou descoberta com seleção condicional.
///
/// # Erros
/// Retorna diagnósticos localizados de configuração, diretiva ou arquivo ativo.
pub fn load_with_config_and_environment(
    entry: &Path,
    package_config: Option<&Path>,
    environment: &CompilationEnvironment,
) -> Result<SourceGraph, GraphError> {
    let path =
        std::fs::canonicalize(entry).map_err(|error| error_at(entry, None, error.to_string()))?;
    let config = config::Config::load(&path, package_config)?;
    let source =
        std::fs::read_to_string(&path).map_err(|error| error_at(&path, None, error.to_string()))?;
    let mut known = HashMap::from([(path.clone(), 0)]);
    let mut units = vec![SourceUnit {
        path,
        source,
        imports: vec![],
        exports: vec![],
    }];
    let mut core_unit = None;
    let mut current = 0;
    while current < units.len() {
        let path = units[current].path.clone();
        let directives = extract(&units[current].source, &path)?;
        for Directive {
            mut uri,
            alternatives,
            span,
            combinators,
            export,
        } in directives
        {
            if let Some(alternative) = alternatives.into_iter().find(|alternative| {
                environment.condition(&alternative.name, alternative.expected.as_deref())
            }) {
                uri = alternative.uri;
            }
            config::validate_uri(&uri).map_err(|message| error_at(&path, Some(span), message))?;
            if uri == "dart:core" {
                if export || !combinators.is_empty() {
                    return Err(error_at(
                        &path,
                        Some(span),
                        "dart:core com export/show/hide ainda não suportado".into(),
                    ));
                }
                let target = *core_unit.get_or_insert_with(|| {
                    let id = units.len();
                    units.push(SourceUnit {
                        path: PathBuf::from("dart:core"),
                        source: String::new(),
                        imports: vec![],
                        exports: vec![],
                    });
                    id
                });
                units[current].imports.push(Import {
                    uri,
                    target,
                    span,
                    combinators,
                });
                continue;
            }
            let candidate = config
                .resolve(&path, &uri)
                .map_err(|message| error_at(&path, Some(span), message))?;
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
                    exports: vec![],
                });
                id
            };
            let edge = Import {
                uri,
                target,
                span,
                combinators,
            };
            if export {
                units[current].exports.push(edge);
            } else {
                units[current].imports.push(edge);
            }
        }
        current += 1;
    }
    Ok(SourceGraph {
        units,
        entry: 0,
        environment: environment.clone(),
    })
}

/// Constrói diagnóstico sem misturar spans de arquivos diferentes.
fn error_at(path: &Path, span: Option<Span>, message: String) -> GraphError {
    GraphError {
        path: path.to_owned(),
        span,
        message,
    }
}

/// Diretiva sintática antes da resolução do destino.
struct Directive {
    uri: String,
    alternatives: Vec<ConditionalUri>,
    span: Span,
    combinators: Vec<Combinator>,
    export: bool,
}
/// Alternativa sintática, mantida sem resolver ou acessar seu destino.
struct ConditionalUri {
    name: String,
    expected: Option<String>,
    uri: String,
}

/// Consome pontuação da diretiva e conserva o intervalo do token inesperado.
fn expect_directive(
    tokens: &[Token<'_>],
    index: &mut usize,
    expected: TokenKind<'_>,
    end: usize,
    path: &Path,
) -> Result<(), GraphError> {
    if tokens.get(*index).map(|token| token.kind) != Some(expected) {
        return Err(error_at(
            path,
            Some(token_span(tokens, *index, end)),
            format!("diretiva exige {expected:?}"),
        ));
    }
    *index += 1;
    Ok(())
}

/// Decodifica uma string da diretiva; formas raw preservam barras literalmente.
fn directive_string(
    tokens: &[Token<'_>],
    index: &mut usize,
    end: usize,
    path: &Path,
) -> Result<String, GraphError> {
    let span = token_span(tokens, *index, end);
    let value = match tokens.get(*index).map(|token| token.kind) {
        Some(TokenKind::RawString(value)) => value.to_owned(),
        Some(TokenKind::String(value)) => decode_directive_string(value, span)
            .map_err(|error| error_at(path, Some(error.span), error.message))?,
        _ => {
            return Err(error_at(
                path,
                Some(span),
                "diretiva exige string literal".into(),
            ));
        }
    };
    *index += 1;
    Ok(value)
}

/// Calcula o fim do prefixo de diretivas sem selecionar ou resolver bibliotecas.
///
/// # Erros
/// Retorna diagnóstico léxico ou sintático da unidade, com spans da fonte original.
///
///     let fonte = "import 'a.dart' if (dart.library.io) 'b.dart';";
///     assert_eq!(dartforge_packages::directive_prefix_end(fonte).unwrap(), fonte.len());
pub fn directive_prefix_end(source: &str) -> Result<usize, Diagnostic> {
    extract(source, Path::new("<source>"))
        .map(|directives| directives.last().map_or(0, |directive| directive.span.end))
        .map_err(|error| {
            Diagnostic::new(
                error.message,
                error.span.unwrap_or(Span { start: 0, end: 0 }),
            )
        })
}

/// Extrai imports e exports do prefixo, preservando combinadores sequenciais.
fn extract(source: &str, path: &Path) -> Result<Vec<Directive>, GraphError> {
    let tokens = dartforge_lexer::lex(source)
        .map_err(|error| error_at(path, Some(error.span), error.message))?;
    validate_language_version(source)
        .map_err(|error| error_at(path, Some(error.span), error.message))?;
    let mut index = 0;
    let mut directives = Vec::new();
    while matches!(
        tokens.get(index).map(|t| t.kind),
        Some(TokenKind::Word("import" | "export"))
    ) {
        let export = tokens[index].kind == TokenKind::Word("export");
        let start = tokens[index].span.start;
        index += 1;
        let uri = directive_string(&tokens, &mut index, source.len(), path)?;
        let mut alternatives = Vec::new();
        while tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("if")) {
            index += 1;
            expect_directive(
                &tokens,
                &mut index,
                TokenKind::Symbol('('),
                source.len(),
                path,
            )?;
            let mut name = String::new();
            loop {
                let Some(TokenKind::Word(part)) = tokens.get(index).map(|t| t.kind) else {
                    return Err(error_at(
                        path,
                        Some(token_span(&tokens, index, source.len())),
                        "condição exige identificador pontuado".into(),
                    ));
                };
                if reserved_combinator(part) {
                    return Err(error_at(
                        path,
                        Some(tokens[index].span),
                        "identificador inválido na condição".into(),
                    ));
                }
                name.push_str(part);
                index += 1;
                if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Symbol('.')) {
                    break;
                }
                name.push('.');
                index += 1;
            }
            let expected = if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("==")) {
                index += 1;
                Some(directive_string(&tokens, &mut index, source.len(), path)?)
            } else {
                None
            };
            expect_directive(
                &tokens,
                &mut index,
                TokenKind::Symbol(')'),
                source.len(),
                path,
            )?;
            let uri = directive_string(&tokens, &mut index, source.len(), path)?;
            alternatives.push(ConditionalUri {
                name,
                expected,
                uri,
            });
        }
        let mut combinators = Vec::new();
        while let Some(TokenKind::Word(kind @ ("show" | "hide"))) =
            tokens.get(index).map(|t| t.kind)
        {
            index += 1;
            let mut names = Vec::new();
            loop {
                let Some(TokenKind::Word(name)) = tokens.get(index).map(|t| t.kind) else {
                    return Err(error_at(
                        path,
                        Some(token_span(&tokens, index, source.len())),
                        "combinador exige identificador".into(),
                    ));
                };
                if reserved_combinator(name) {
                    return Err(error_at(
                        path,
                        Some(tokens[index].span),
                        "identificador de combinador inválido".into(),
                    ));
                }
                names.push(name.to_owned());
                index += 1;
                if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Symbol(',')) {
                    break;
                }
                index += 1;
            }
            combinators.push(if kind == "show" {
                Combinator::Show(names)
            } else {
                Combinator::Hide(names)
            });
        }
        if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Symbol(';')) {
            return Err(error_at(
                path,
                Some(token_span(&tokens, index, source.len())),
                "diretiva exige ; (as/deferred não suportados)".into(),
            ));
        }
        directives.push(Directive {
            uri,
            alternatives,
            combinators,
            export,
            span: Span {
                start,
                end: tokens[index].span.end,
            },
        });
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
                    format!("diretiva {word} fora do prefixo suportado"),
                ));
            }
            _ => {}
        }
    }
    Ok(directives)
}
/// Decodifica escapes Dart em unidades UTF-16 e rejeita surrogates isolados.
fn decode_directive_string(text: &str, span: Span) -> Result<String, Diagnostic> {
    let mut chars = text.chars();
    let mut units = Vec::with_capacity(text.len());
    while let Some(mut ch) = chars.next() {
        if ch == '\\' {
            ch = chars
                .next()
                .ok_or_else(|| Diagnostic::new("truncated string escape", span))?;
            match ch {
                'b' => ch = '\u{8}',
                'f' => ch = '\u{c}',
                'n' => ch = '\n',
                'r' => ch = '\r',
                't' => ch = '\t',
                'v' => ch = '\u{b}',
                'x' | 'u' => {
                    let value = if ch == 'x' {
                        hex_digits(&mut chars, 2, span)?
                    } else if chars.clone().next() == Some('{') {
                        chars.next();
                        let mut value = 0u32;
                        let mut count = 0;
                        loop {
                            let c = chars.next().ok_or_else(|| {
                                Diagnostic::new("unterminated Unicode escape", span)
                            })?;
                            if c == '}' {
                                if count == 0 {
                                    return Err(Diagnostic::new(
                                        "Unicode escape requires 1 to 6 hexadecimal digits",
                                        span,
                                    ));
                                }
                                break;
                            }
                            let digit =
                                c.to_digit(16).filter(|_| c.is_ascii()).ok_or_else(|| {
                                    Diagnostic::new(
                                        "invalid hexadecimal digit in Unicode escape",
                                        span,
                                    )
                                })?;
                            count += 1;
                            if count > 6 {
                                return Err(Diagnostic::new(
                                    "Unicode escape requires 1 to 6 hexadecimal digits",
                                    span,
                                ));
                            }
                            value = value * 16 + digit;
                        }
                        value
                    } else {
                        hex_digits(&mut chars, 4, span)?
                    };
                    if value > 0x10FFFF {
                        return Err(Diagnostic::new("Unicode escape exceeds U+10FFFF", span));
                    }
                    if value <= 0xFFFF {
                        units.push(value as u16);
                    } else {
                        let value = value - 0x10000;
                        units.push(0xD800 + (value >> 10) as u16);
                        units.push(0xDC00 + (value & 0x3FF) as u16);
                    }
                    continue;
                }
                // Dart remove a barra invertida de escapes desconhecidos.
                _ => {}
            }
        }
        let mut encoded = [0u16; 2];
        units.extend_from_slice(ch.encode_utf16(&mut encoded));
    }
    String::from_utf16(&units)
        .map_err(|_| Diagnostic::new("isolated UTF-16 surrogates are not supported", span))
}

/// Lê a quantidade exata de dígitos hexadecimais de um escape fixo.
fn hex_digits(
    chars: &mut std::str::Chars<'_>,
    count: usize,
    span: Span,
) -> Result<u32, Diagnostic> {
    let mut value = 0;
    for _ in 0..count {
        let digit = chars
            .next()
            .and_then(|c| if c.is_ascii() { c.to_digit(16) } else { None })
            .ok_or_else(|| {
                Diagnostic::new(
                    "escape requires the exact number of hexadecimal digits",
                    span,
                )
            })?;
        value = value * 16 + digit;
    }
    Ok(value)
}
/// Respeita comentários aninhados e examina versões somente antes do primeiro token.
///
/// # Erros
/// Retorna diagnóstico quando um marcador válido seleciona versão diferente de 3.6.
///
///     assert!(dartforge_packages::validate_language_version("// @dart = 3.6\nvoid main(){}").is_ok());
pub fn validate_language_version(prefix: &str) -> Result<(), Diagnostic> {
    let bytes = prefix.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes.get(i..i + 2) == Some(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                if bytes.get(i..i + 2) == Some(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes.get(i..i + 2) == Some(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if bytes.get(i..i + 2) == Some(b"//") {
            let start = i;
            i += 2;
            let content = i;
            while i < bytes.len() && !matches!(bytes[i], b'\r' | b'\n') {
                i += 1;
            }
            let comment = prefix[content..i].trim_matches(' ');
            if let Some(version) = comment
                .strip_prefix("@dart")
                .and_then(|s| s.trim_start_matches(' ').strip_prefix('='))
                .map(|s| s.trim_matches(' '))
                && let Some((major, minor)) = version.split_once('.')
                && !major.is_empty()
                && !minor.is_empty()
                && major
                    .bytes()
                    .chain(minor.bytes())
                    .all(|b| b.is_ascii_digit())
                && (major.parse::<u32>().ok() != Some(3) || minor.parse::<u32>().ok() != Some(6))
            {
                return Err(Diagnostic::new(
                    "versão @dart não suportada; somente 3.6 foi verificada",
                    Span { start, end: i },
                ));
            }
        } else if bytes[i].is_ascii_whitespace() {
            i += 1;
        } else {
            break;
        }
    }
    Ok(())
}

/// Rejeita palavras reservadas reais sem confundir identificadores contextuais show/hide.
fn reserved_combinator(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "else"
            | "enum"
            | "extends"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "if"
            | "in"
            | "is"
            | "new"
            | "null"
            | "rethrow"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "var"
            | "void"
            | "while"
            | "with"
            | "import"
            | "export"
    )
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

    /// A primeira condição verdadeira vence sem abrir alternativas inativas.
    #[test]
    fn conditional_first_match_and_inactive_destinations() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'missing.dart' if (dart.library.core) 'first.dart' if (dart.library.core) 'dart:io'; export 'dart:unsupported' if (dart.library.core == r'true') 'first.dart' show value;");
        fixture.write("first.dart", "int value()=>1;");
        let graph = load(&entry).unwrap();
        assert_eq!(graph.units.len(), 2);
        assert_eq!(graph.units[0].imports[0].uri, "first.dart");
        assert_eq!(graph.units[0].exports[0].target, 1);
        assert_eq!(
            graph.units[0].exports[0].combinators,
            [Combinator::Show(vec!["value".into()])]
        );
        assert_eq!(graph.environment, CompilationEnvironment::javascript());
    }

    /// Strings raw e escapes são decodificados sem depender da seleção do destino.
    #[test]
    fn conditional_strings_and_prefix_are_target_independent() {
        let fixture = Fixture::new();
        let directive = r"import 'missing.dart' if (dart.library.core == 'tr\u0075e') 'a\x2edart' if (absent) r'missing\raw.dart';";
        let source = format!("{directive} void main(){{}}");
        let entry = fixture.write("main.dart", &source);
        fixture.write("a.dart", "");
        assert_eq!(load(&entry).unwrap().units[0].imports[0].uri, "a.dart");
        assert_eq!(directive_prefix_end(&source).unwrap(), directive.len());
        let inactive = "import 'dart:unsupported' if (absent) 'missing.dart';";
        assert_eq!(directive_prefix_end(inactive).unwrap(), inactive.len());
        for source in [
            "import 'a.dart' if (key == true) 'b.dart';",
            "import 'a.dart' if (key.) 'b.dart';",
            "import 'a.dart' if (key != 'true') 'b.dart';",
            "import 'a.dart' if (key) ;",
            r"import 'a.dart' if (key == '\xG0') 'b.dart';",
            "import 'a.dart' show A if (key) 'b.dart';",
            "import 'a.dart' if (class) 'b.dart';",
        ] {
            let error = directive_prefix_end(source).unwrap_err();
            assert!(
                error.span.start <= error.span.end && error.span.end <= source.len(),
                "{source}"
            );
        }
    }

    /// Ambiente integra a identidade do grafo mesmo quando não muda as arestas.
    #[test]
    fn environment_identity_and_absent_conditions() {
        let fixture = Fixture::new();
        let entry = fixture.write(
            "main.dart",
            "import 'a.dart' if (missing.key) 'missing.dart';",
        );
        fixture.write("a.dart", "");
        let js = load_with_environment(&entry, &CompilationEnvironment::javascript()).unwrap();
        let native = load_with_environment(&entry, &CompilationEnvironment::native()).unwrap();
        assert_eq!(js.units, native.units);
        assert_ne!(js, native);
    }

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

    /// Raízes aninhadas são permitidas somente fora do diretório público do ancestral.
    #[test]
    fn package_root_nesting_rules() {
        let f = Fixture::new();
        let entry = f.write("main.dart", "void main(){}");
        for (package_uri, child_root, valid) in [
            ("lib/", "../root/tools/child/", true),
            ("lib/", "../root/lib/child/", false),
            ("tools/child/lib/", "../root/tools/child/", false),
        ] {
            let text = serde_json::json!({"configVersion":2,"packages":[
                {"name":"parent","rootUri":"../root/","packageUri":package_uri},
                {"name":"child","rootUri":child_root,"packageUri":"lib/"}
            ]})
            .to_string();
            let path = f.write(".dart_tool/package_config.json", &text);
            assert_eq!(
                load_with_config(&entry, Some(&path)).is_ok(),
                valid,
                "{text}"
            );
        }
    }
    /// Ignora marcadores dentro de blocos, doc comments ou depois de declarações.
    #[test]
    fn language_comment_uses_lexical_preamble() {
        let f = Fixture::new();
        for source in [
            "/* // @dart = 2.9 */ void main(){}",
            "/* outer /* nested */\n// @dart = 2.9\n*/ void main(){}",
            "void main(){}\n// @dart = 2.9",
            "/// @dart = 2.9\nvoid main(){}",
            "// @dart = 3.6 trailing text\nvoid main(){}",
            "// @dart = 03.06\nvoid main(){}",
        ] {
            let entry = f.write("main.dart", source);
            assert!(load(&entry).is_ok(), "{source}");
        }
        let entry = f.write("main.dart", "/* header */ // @dart = 2.9\nvoid main(){}");
        assert!(load(&entry).unwrap_err().message.contains("@dart"));
        f.write("other.dart", "int x(){return 1;}");
        for keyword in ["return", "class", "true"] {
            let entry = f.write("main.dart", &format!("import 'other.dart' show {keyword};"));
            assert!(load(&entry).is_err());
        }
    }
    /// Descobre a configuração ancestral e resolve espaços, Unicode e diretório padrão.
    #[test]
    fn package_config_discovery_uri_encoding_and_defaults() {
        let f = Fixture::new();
        let entry = f.write(
            "app/bin/main.dart",
            "import 'package:p/a.dart' show A hide B; export 'package:q/b.dart' show B;",
        );
        f.write("dep espaço/lib/a.dart", "class A {}");
        f.write("outro/b.dart", "class B {}");
        f.write("app/.dart_tool/package_config.json",r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../../dep%20espa%C3%A7o","packageUri":"lib","languageVersion":"3.6"},{"name":"q","rootUri":"../../outro"}]}"#);
        let graph = load(&entry).unwrap();
        assert_eq!(graph.units.len(), 3);
        assert_eq!(
            graph.units[0].imports[0].combinators,
            [
                Combinator::Show(vec!["A".into()]),
                Combinator::Hide(vec!["B".into()])
            ]
        );
        assert_eq!(graph.units[0].exports.len(), 1);
        assert_eq!(graph, load(&entry).unwrap());
    }

    /// Configuração explícita file URI funciona e um remapeamento aparece na carga seguinte.
    #[test]
    fn explicit_file_uri_and_config_remap() {
        let f = Fixture::new();
        let entry = f.write("src/main.dart", "import 'package:p/a.dart';");
        f.write("one/lib/a.dart", "int a(){return 1;}");
        f.write("two/lib/a.dart", "int a(){return 1;}");
        let config = f.0.join("chosen.json");
        let save = |dir: &str| {
            let root = url::Url::from_directory_path(f.0.join(dir))
                .unwrap()
                .to_string();
            std::fs::write(&config,serde_json::json!({"configVersion":2,"packages":[{"name":"p","rootUri":root,"packageUri":"lib/"}]}).to_string()).unwrap();
        };
        save("one");
        let first = load_with_config(&entry, Some(&config)).unwrap();
        save("two");
        let second = load_with_config(&entry, Some(&config)).unwrap();
        assert_ne!(first.units[1].path, second.units[1].path);
        assert_eq!(first.units[1].source, second.units[1].source);
    }

    /// Configurações malformadas e versões não verificadas falham antes de compilar.
    #[test]
    fn invalid_configs_and_language_gates() {
        let f = Fixture::new();
        let entry = f.write("main.dart", "void main(){}");
        for config in [
            r#"{"configVersion":3,"packages":[]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../x","packageUri":"../outside"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../x","languageVersion":"3.7"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../x","languageVersion":"2.12"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../x","languageVersion":"03.6"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../x"},{"name":"p","rootUri":"../y"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../x"},{"name":"q","rootUri":"../x"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"http://example.com/x"}]}"#,
            r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"bad%xx"}]}"#,
        ] {
            let path = f.write("config.json", config);
            assert!(load_with_config(&entry, Some(&path)).is_err(), "{config}");
        }
        let versioned = f.write("old.dart", "// @dart = 2.9\nvoid main(){}");
        assert!(load(&versioned).unwrap_err().message.contains("@dart"));
        let source = f.write("unknown.dart", "import 'package:unknown/a.dart';");
        assert!(load(&source).unwrap_err().message.contains("desconhecido"));
    }

    /// Exports e combinadores repetidos preservam sua ordem para o linker.
    #[test]
    fn repeated_combinators_and_relative_encoded_uri() {
        let f = Fixture::new();
        let entry = f.write(
            "main.dart",
            "export 'a%20b.dart' show A,B show A hide B; import 'a%20b.dart';",
        );
        f.write("a b.dart", "class A{} class B{}");
        let graph = load(&entry).unwrap();
        assert_eq!(graph.units.len(), 2);
        assert_eq!(graph.units[0].exports[0].combinators.len(), 3);
        for source in [
            "import 'a%20b.dart' show;",
            "export 'a%20b.dart' hide A,;",
            "import 'a%20b.dart' as p;",
        ] {
            f.write("main.dart", source);
            assert!(load(&entry).is_err());
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
            "import 'dart:io';",
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
