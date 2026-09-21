//! Carrega arquivos Dart, imports/exports locais e package_config v2 do subconjunto.
//!
//! Não resolve símbolos, bibliotecas, privacidade ou namespaces, nem combina a
//! saída de múltiplos arquivos. O lexer existente valida a tokenização inteira.
//! Condicionais selecionam a primeira alternativa verdadeira; destinos inativos
//! não são resolvidos nem lidos. A análise do prefixo independe do perfil alvo.
//! Diretivas `part`/`part of` também são resolvidas: a parte vira uma unidade própria
//! ligada ao pai, sem imports, exports ou escopo próprios.
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
    /// Arquivo de package_config lido e seu conteúdo exato, quando existe.
    ///
    /// Registrado para que a revalidação incremental possa provar que a
    /// resolução de `package:` não mudou sem recarregar o grafo inteiro.
    pub config_origin: Option<(PathBuf, String)>,
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
    /// Partes declaradas por `part` na ordem textual; sempre vazio em uma parte.
    pub parts: Vec<Part>,
    /// Unidade da biblioteca que declara este arquivo como parte, quando houver.
    pub part_of: Option<usize>,
    /// Fim em bytes do prefixo de diretivas, inclusive library, part e part of.
    pub directives_end: usize,
}
impl SourceUnit {
    /// Cria a unidade antes de resolver as diretivas do próprio arquivo.
    fn new(path: PathBuf, source: String, part_of: Option<usize>) -> Self {
        Self {
            path,
            source,
            imports: vec![],
            exports: vec![],
            parts: vec![],
            part_of,
            directives_end: 0,
        }
    }
}

/// Aresta resolvida de uma diretiva `part 'arquivo.dart';`.
#[derive(Debug, PartialEq, Eq)]
pub struct Part {
    /// Caminho relativo escrito na string da diretiva.
    pub uri: String,
    /// Índice da unidade da parte no grafo.
    pub target: usize,
    /// Intervalo da diretiva completa na biblioteca declarante.
    pub span: Span,
}

/// Aresta resolvida de uma diretiva import.
#[derive(Debug, PartialEq, Eq)]
pub struct Import {
    /// Prefixo explícito, atualmente permitido somente para dart:ffi.
    pub prefix: Option<String>,
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
/// Aceita dart:core sem filtros, `library`, `part` e `part of`; outras bibliotecas
/// dart: e aliases são rejeitados. Partes não podem ser importadas nem reivindicadas duas vezes.
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
    let mut units = vec![SourceUnit::new(path, source, None)];
    // Papel e nome declarado acompanham cada unidade para reivindicar arquivos uma vez.
    let mut roles = vec![Role::Library];
    let mut library_names: Vec<Option<String>> = vec![None];
    let mut sdk_units = HashMap::new();
    let mut current = 0;
    while current < units.len() {
        let path = units[current].path.clone();
        let directives = extract(&units[current].source, &path)?;
        units[current].directives_end = directives.end;
        library_names[current] = directives.library.as_ref().map(|(name, _)| name.clone());
        if let Role::Part { owner } = roles[current] {
            validate_part(&config, &units, &library_names, current, owner, &directives)?;
            current += 1;
            continue;
        }
        if let Some((_, span)) = &directives.part_of {
            return Err(error_at(
                &path,
                Some(*span),
                "part of exige uma biblioteca que declare part para este arquivo".into(),
            ));
        }
        // Canonicalização e leitura são syscalls independentes: resolver as URIs
        // é aritmética de caminhos, então todos os alvos desta unidade podem ser
        // buscados de uma vez. Erros ficam guardados e são relatados no mesmo
        // ponto e na mesma ordem do laço sequencial abaixo.
        let prefetched = prefetch_imports(&config, &path, environment, &directives.imports, &known);
        for Directive {
            mut uri,
            alternatives,
            span,
            combinators,
            export,
            prefix,
        } in directives.imports
        {
            if let Some(alternative) = alternatives.into_iter().find(|alternative| {
                environment.condition(&alternative.name, alternative.expected.as_deref())
            }) {
                uri = alternative.uri;
            }
            config::validate_uri(&uri).map_err(|message| error_at(&path, Some(span), message))?;
            if prefix.is_some() && uri != "dart:ffi" {
                return Err(error_at(
                    &path,
                    Some(span),
                    "prefixos fora de dart:ffi ainda não suportados".into(),
                ));
            }
            if matches!(uri.as_str(), "dart:core" | "dart:ffi" | "dart:async") {
                if uri != "dart:async" && (export || !combinators.is_empty()) {
                    return Err(error_at(
                        &path,
                        Some(span),
                        "biblioteca SDK com export/show/hide ainda não suportada".into(),
                    ));
                }
                if uri == "dart:ffi" && environment.target() != CompilationTarget::Native {
                    return Err(error_at(
                        &path,
                        Some(span),
                        "dart:ffi exige o backend Native AOT".into(),
                    ));
                }
                let target = *sdk_units.entry(uri.clone()).or_insert_with(|| {
                    let id = units.len();
                    units.push(SourceUnit::new(PathBuf::from(&uri), String::new(), None));
                    roles.push(Role::Library);
                    library_names.push(None);
                    id
                });
                let import = Import {
                    prefix,
                    uri,
                    target,
                    span,
                    combinators,
                };
                if export {
                    units[current].exports.push(import);
                } else {
                    units[current].imports.push(import);
                }
                continue;
            }
            let candidate = config
                .resolve(&path, &uri)
                .map_err(|message| error_at(&path, Some(span), message))?;
            let fetched = prefetched.get(&candidate);
            let target_path = match fetched {
                Some(Ok((canonical, _))) => canonical.clone(),
                Some(Err(message)) => {
                    return Err(error_at(&path, Some(span), message.clone()));
                }
                None => std::fs::canonicalize(&candidate).map_err(|error| {
                    error_at(
                        &path,
                        Some(span),
                        format!("não foi possível resolver {}: {error}", candidate.display()),
                    )
                })?,
            };
            let target = if let Some(&id) = known.get(&target_path) {
                id
            } else {
                let source = match fetched {
                    Some(Ok((_, Some(source)))) => source.clone(),
                    _ => std::fs::read_to_string(&target_path).map_err(|error| {
                        error_at(
                            &path,
                            Some(span),
                            format!("não foi possível ler {}: {error}", target_path.display()),
                        )
                    })?,
                };
                let id = units.len();
                known.insert(target_path.clone(), id);
                units.push(SourceUnit::new(target_path, source, None));
                roles.push(Role::Library);
                library_names.push(None);
                id
            };
            if matches!(roles[target], Role::Part { .. }) {
                return Err(error_at(
                    &path,
                    Some(span),
                    format!("{uri} é parte de outra biblioteca e não pode ser importada"),
                ));
            }
            let edge = Import {
                prefix,
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
        for PartDirective { uri, span } in directives.parts {
            config::validate_uri(&uri).map_err(|message| error_at(&path, Some(span), message))?;
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
            if let Some(&id) = known.get(&target_path) {
                return Err(error_at(
                    &path,
                    Some(span),
                    claimed_message(&roles, id, current),
                ));
            }
            let source = std::fs::read_to_string(&target_path).map_err(|error| {
                error_at(
                    &path,
                    Some(span),
                    format!("não foi possível ler {}: {error}", target_path.display()),
                )
            })?;
            let target = units.len();
            known.insert(target_path.clone(), target);
            units.push(SourceUnit::new(target_path, source, Some(current)));
            roles.push(Role::Part { owner: current });
            library_names.push(None);
            units[current].parts.push(Part { uri, target, span });
        }
        current += 1;
    }
    Ok(SourceGraph {
        units,
        entry: 0,
        environment: environment.clone(),
        config_origin: config.origin.clone(),
    })
}

/// Reaproveita a estrutura de um grafo quando só os corpos dos arquivos mudaram.
///
/// Relê **todos** os arquivos já conhecidos, em paralelo, e compara byte a byte.
/// Não usa mtime nem tamanho: a garantia de que uma edição com mtime restaurado
/// e tamanho idêntico invalida o cache continua valendo.
///
/// A estrutura só é reaproveitada quando, para cada arquivo alterado, as
/// diretivas continuam **estruturalmente idênticas** — mesmas URIs na mesma
/// ordem, mesmos combinadores, mesmos prefixos, mesmos `part` e o mesmo
/// `part of`. Como a resolução é função pura de diretivas, configuração e
/// ambiente, isso prova que as arestas e os IDs das unidades são os mesmos.
/// Os spans e o fim do prefixo de diretivas são atualizados a partir do texto
/// novo, porque é esse texto que o front-end vai analisar.
///
/// Devolve `None` sempre que houver qualquer dúvida: arquivo ausente, diretiva
/// nova ou removida, configuração de pacotes alterada, ambiente diferente ou
/// entrada apontando para outro arquivo. Nesses casos o chamador deve recarregar
/// o grafo do zero.
///
/// # Exemplos
///
/// ```no_run
/// # use dartforge_packages::{load, revalidate, CompilationEnvironment};
/// let antes = load(std::path::Path::new("lib/main.dart"))?;
/// let ambiente = CompilationEnvironment::javascript();
/// match revalidate(&antes, std::path::Path::new("lib/main.dart"), &ambiente) {
///     Some(agora) => assert_eq!(agora.units.len(), antes.units.len()),
///     None => { /* estrutura mudou: recarregar */ }
/// }
/// # Ok::<(), dartforge_packages::GraphError>(())
/// ```
pub fn revalidate(
    graph: &SourceGraph,
    entry: &Path,
    environment: &CompilationEnvironment,
) -> Option<SourceGraph> {
    if graph.environment != *environment || graph.entry != 0 || graph.units.is_empty() {
        return None;
    }
    if std::fs::canonicalize(entry).ok()? != graph.units[0].path {
        return None;
    }
    if let Some((path, previous)) = &graph.config_origin
        && std::fs::read_to_string(path).ok()?.as_str() != previous.as_str()
    {
        return None;
    }
    let sources = read_known_sources(graph)?;
    let mut units = Vec::with_capacity(graph.units.len());
    for (unit, source) in graph.units.iter().zip(sources) {
        let Some(source) = source else {
            // Unidade sintética de biblioteca SDK: não tem arquivo em disco.
            units.push(SourceUnit {
                path: unit.path.clone(),
                source: String::new(),
                imports: clone_imports(&unit.imports),
                exports: clone_imports(&unit.exports),
                parts: clone_parts(&unit.parts),
                part_of: unit.part_of,
                directives_end: unit.directives_end,
            });
            continue;
        };
        if source == unit.source {
            units.push(SourceUnit {
                path: unit.path.clone(),
                source,
                imports: clone_imports(&unit.imports),
                exports: clone_imports(&unit.exports),
                parts: clone_parts(&unit.parts),
                part_of: unit.part_of,
                directives_end: unit.directives_end,
            });
            continue;
        }
        let directives = extract(&source, &unit.path).ok()?;
        let updated = rebind_directives(unit, &directives)?;
        units.push(SourceUnit {
            path: unit.path.clone(),
            source,
            directives_end: directives.end,
            ..updated
        });
    }
    Some(SourceGraph {
        units,
        entry: 0,
        environment: environment.clone(),
        config_origin: graph.config_origin.clone(),
    })
}

/// Lê em paralelo os arquivos do grafo; `None` marca unidade sintética de SDK.
///
/// Qualquer falha de leitura devolve `None` para o grafo inteiro: um arquivo que
/// sumiu muda a resolução e exige a carga completa, com o diagnóstico correto.
fn read_known_sources(graph: &SourceGraph) -> Option<Vec<Option<String>>> {
    let read = |unit: &SourceUnit| {
        if unit.path.to_str().is_some_and(|p| p.starts_with("dart:")) {
            return Some(None);
        }
        std::fs::read_to_string(&unit.path).ok().map(Some)
    };
    let results: Vec<Option<Option<String>>> = if graph.units.len() < PREFETCH_THRESHOLD {
        graph.units.iter().map(read).collect()
    } else {
        use rayon::prelude::*;
        graph.units.par_iter().map(read).collect()
    };
    results.into_iter().collect()
}

/// Copia arestas preservando alvo, filtros e prefixo.
fn clone_imports(imports: &[Import]) -> Vec<Import> {
    imports
        .iter()
        .map(|import| Import {
            prefix: import.prefix.clone(),
            uri: import.uri.clone(),
            target: import.target,
            span: import.span,
            combinators: import.combinators.clone(),
        })
        .collect()
}

/// Copia declarações `part` preservando o alvo já resolvido.
fn clone_parts(parts: &[Part]) -> Vec<Part> {
    parts
        .iter()
        .map(|part| Part {
            uri: part.uri.clone(),
            target: part.target,
            span: part.span,
        })
        .collect()
}

/// Confere que as diretivas relidas são as mesmas e adota os spans novos.
///
/// Devolve `None` na menor divergência estrutural. Em particular, uma diretiva
/// acrescentada depois das anteriores muda a contagem e reprova aqui, que é
/// exatamente o caso que uma comparação só do prefixo deixaria passar.
fn rebind_directives(unit: &SourceUnit, directives: &UnitDirectives) -> Option<SourceUnit> {
    if unit.part_of.is_some() != directives.part_of.is_some()
        || unit.parts.len() != directives.parts.len()
    {
        return None;
    }
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    // As arestas ficam em vetores separados, mas cada um preserva a ordem
    // textual: consumir o vetor correspondente ao tipo da diretiva reconstrói
    // a correspondência mesmo quando import e export estão intercalados.
    for directive in &directives.imports {
        let existing = if directive.export {
            unit.exports.get(exports.len())?
        } else {
            unit.imports.get(imports.len())?
        };
        if existing.uri != directive.uri
            || existing.prefix != directive.prefix
            || existing.combinators != directive.combinators
            || !directive.alternatives.is_empty()
        {
            return None;
        }
        let rebound = Import {
            prefix: existing.prefix.clone(),
            uri: existing.uri.clone(),
            target: existing.target,
            span: directive.span,
            combinators: existing.combinators.clone(),
        };
        if directive.export {
            exports.push(rebound);
        } else {
            imports.push(rebound);
        }
    }
    if imports.len() != unit.imports.len() || exports.len() != unit.exports.len() {
        return None;
    }
    let mut parts = Vec::new();
    for (existing, fresh) in unit.parts.iter().zip(&directives.parts) {
        if existing.uri != fresh.uri {
            return None;
        }
        parts.push(Part {
            uri: existing.uri.clone(),
            target: existing.target,
            span: fresh.span,
        });
    }
    Some(SourceUnit {
        path: unit.path.clone(),
        source: String::new(),
        imports,
        exports,
        parts,
        part_of: unit.part_of,
        directives_end: directives.end,
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

/// Papel já atribuído a uma unidade descoberta, usado para reivindicar arquivos.
#[derive(Clone, Copy)]
enum Role {
    /// Arquivo com escopo próprio: entrada, import ou export.
    Library,
    /// Arquivo reivindicado por uma única biblioteca através de `part`.
    Part { owner: usize },
}

/// Explica por que o arquivo apontado por `part` já pertence a outra unidade.
fn claimed_message(roles: &[Role], target: usize, current: usize) -> String {
    match roles[target] {
        Role::Library if target == current => {
            "uma biblioteca não pode declarar a si mesma como parte".into()
        }
        Role::Library => "arquivo já carregado como biblioteca não pode ser parte".into(),
        Role::Part { owner } if owner == current => {
            "part duplicado para o mesmo arquivo nesta biblioteca".into()
        }
        Role::Part { .. } => "arquivo já é parte de outra biblioteca".into(),
    }
}

/// Valida as restrições da parte e sua correspondência com a biblioteca declarante.
fn validate_part(
    config: &config::Config,
    units: &[SourceUnit],
    library_names: &[Option<String>],
    current: usize,
    owner: usize,
    directives: &UnitDirectives,
) -> Result<(), GraphError> {
    let path = &units[current].path;
    if let Some(directive) = directives.imports.first() {
        return Err(error_at(
            path,
            Some(directive.span),
            "parte não pode declarar import ou export".into(),
        ));
    }
    if let Some(part) = directives.parts.first() {
        return Err(error_at(
            path,
            Some(part.span),
            "parte não pode declarar part".into(),
        ));
    }
    if let Some((_, span)) = &directives.library {
        return Err(error_at(
            path,
            Some(*span),
            "parte não pode declarar library".into(),
        ));
    }
    let parent = &units[owner].path;
    let Some((target, span)) = &directives.part_of else {
        return Err(error_at(
            path,
            Some(Span { start: 0, end: 0 }),
            format!(
                "arquivo incluído por part exige part of de {}",
                parent.display()
            ),
        ));
    };
    match target {
        PartOf::Uri(uri) => {
            config::validate_uri(uri).map_err(|message| error_at(path, Some(*span), message))?;
            let candidate = config
                .resolve(path, uri)
                .map_err(|message| error_at(path, Some(*span), message))?;
            let resolved = std::fs::canonicalize(&candidate).map_err(|error| {
                error_at(
                    path,
                    Some(*span),
                    format!("não foi possível resolver {}: {error}", candidate.display()),
                )
            })?;
            if &resolved != parent {
                return Err(error_at(
                    path,
                    Some(*span),
                    format!(
                        "part of aponta para {} em vez de {}",
                        resolved.display(),
                        parent.display()
                    ),
                ));
            }
        }
        PartOf::Name(name) => {
            if library_names[owner].as_deref() != Some(name.as_str()) {
                return Err(error_at(
                    path,
                    Some(*span),
                    format!(
                        "part of {name} não corresponde à biblioteca {}",
                        parent.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Prefixo completo de diretivas de uma unidade, antes de resolver os destinos.
struct UnitDirectives {
    /// Nome declarado por `library nome;`, consultado por `part of nome;`.
    library: Option<(String, Span)>,
    /// Alvo declarado por `part of ...;` quando o arquivo é uma parte.
    part_of: Option<(PartOf, Span)>,
    /// Imports e exports na ordem textual.
    imports: Vec<Directive>,
    /// Partes declaradas depois dos imports, na ordem textual.
    parts: Vec<PartDirective>,
    /// Fim em bytes da última diretiva reconhecida.
    end: usize,
}

/// Forma aceita em `part of`: URI relativa do pai ou nome de biblioteca.
enum PartOf {
    /// URI relativa que deve resolver exatamente para o arquivo pai.
    Uri(String),
    /// Nome pontuado que deve coincidir com o `library` do pai.
    Name(String),
}

/// Diretiva `part 'arquivo.dart';` antes de resolver o destino.
struct PartDirective {
    /// Caminho relativo escrito na string da diretiva.
    uri: String,
    /// Intervalo da diretiva completa no arquivo declarante.
    span: Span,
}

/// Diretiva sintática antes da resolução do destino.
struct Directive {
    prefix: Option<String>,
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
        .map(|directives| directives.end)
        .map_err(|error| {
            Diagnostic::new(
                error.message,
                error.span.unwrap_or(Span { start: 0, end: 0 }),
            )
        })
}

/// Abaixo deste número de importações o custo de coordenação supera o ganho.
const PREFETCH_THRESHOLD: usize = 4;

/// Canonicaliza e lê, em paralelo, os arquivos importados por uma unidade.
///
/// Só toca arquivos: não interpreta diretivas nem altera o grafo. Cada resultado
/// é indexado pelo caminho candidato antes da canonicalização, de modo que o laço
/// sequencial continue decidindo tudo na ordem escrita — inclusive qual erro é
/// relatado primeiro. Alvos já conhecidos não são relidos; URIs `dart:` e
/// condicionais não selecionadas são ignoradas aqui e resolvidas no laço.
fn prefetch_imports(
    config: &config::Config,
    importer: &Path,
    environment: &CompilationEnvironment,
    imports: &[Directive],
    known: &HashMap<PathBuf, usize>,
) -> HashMap<PathBuf, Result<(PathBuf, Option<String>), String>> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for directive in imports {
        let uri = directive
            .alternatives
            .iter()
            .find(|alternative| {
                environment.condition(&alternative.name, alternative.expected.as_deref())
            })
            .map_or(directive.uri.as_str(), |alternative| {
                alternative.uri.as_str()
            });
        if uri.starts_with("dart:") {
            continue;
        }
        let Ok(candidate) = config.resolve(importer, uri) else {
            continue;
        };
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    if candidates.len() < PREFETCH_THRESHOLD {
        return HashMap::new();
    }
    use rayon::prelude::*;
    candidates
        .par_iter()
        .map(|candidate| {
            let result = match std::fs::canonicalize(candidate) {
                Err(error) => Err(format!(
                    "não foi possível resolver {}: {error}",
                    candidate.display()
                )),
                Ok(canonical) => {
                    if known.contains_key(&canonical) {
                        Ok((canonical, None))
                    } else {
                        match std::fs::read_to_string(&canonical) {
                            Ok(source) => Ok((canonical, Some(source))),
                            Err(error) => Err(format!(
                                "não foi possível ler {}: {error}",
                                canonical.display()
                            )),
                        }
                    }
                }
            };
            (candidate.clone(), result)
        })
        .collect()
}

/// Extrai library, part of, imports/exports e parts na ordem exigida pelo Dart.
fn extract(source: &str, path: &Path) -> Result<UnitDirectives, GraphError> {
    let tokens = dartforge_lexer::lex(source)
        .map_err(|error| error_at(path, Some(error.span), error.message))?;
    validate_language_version(source)
        .map_err(|error| error_at(path, Some(error.span), error.message))?;
    let mut index = 0;
    let mut end = 0;
    let mut library = None;
    let mut part_of = None;
    if tokens.first().map(|token| token.kind) == Some(TokenKind::Word("library")) {
        let start = tokens[0].span.start;
        index = 1;
        let name = dotted_name(&tokens, &mut index, source.len(), path)?;
        expect_directive(
            &tokens,
            &mut index,
            TokenKind::Symbol(';'),
            source.len(),
            path,
        )?;
        end = tokens[index - 1].span.end;
        library = Some((name, Span { start, end }));
    } else if tokens.first().map(|token| token.kind) == Some(TokenKind::Word("part"))
        && tokens.get(1).map(|token| token.kind) == Some(TokenKind::Word("of"))
    {
        let start = tokens[0].span.start;
        index = 2;
        let target = match tokens.get(index).map(|token| token.kind) {
            Some(TokenKind::String(_) | TokenKind::RawString(_)) => {
                PartOf::Uri(directive_string(&tokens, &mut index, source.len(), path)?)
            }
            _ => PartOf::Name(dotted_name(&tokens, &mut index, source.len(), path)?),
        };
        expect_directive(
            &tokens,
            &mut index,
            TokenKind::Symbol(';'),
            source.len(),
            path,
        )?;
        end = tokens[index - 1].span.end;
        part_of = Some((target, Span { start, end }));
    }
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
        let prefix = if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("as")) {
            index += 1;
            let Some(TokenKind::Word(name)) = tokens.get(index).map(|t| t.kind) else {
                return Err(error_at(
                    path,
                    Some(token_span(&tokens, index, source.len())),
                    "prefixo exige identificador".into(),
                ));
            };
            if export || reserved_combinator(name) {
                return Err(error_at(
                    path,
                    Some(tokens[index].span),
                    "prefixo inválido na diretiva".into(),
                ));
            }
            index += 1;
            Some(name.to_owned())
        } else {
            None
        };
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
                "diretiva exige ; (deferred não suportado)".into(),
            ));
        }
        end = tokens[index].span.end;
        directives.push(Directive {
            prefix,
            uri,
            alternatives,
            combinators,
            export,
            span: Span { start, end },
        });
        index += 1;
    }
    let mut parts = Vec::new();
    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Word("part")) {
        let start = tokens[index].span.start;
        if tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::Word("of")) {
            return Err(error_at(
                path,
                Some(tokens[index].span),
                "part of deve ser a primeira diretiva do arquivo".into(),
            ));
        }
        index += 1;
        let uri = directive_string(&tokens, &mut index, source.len(), path)?;
        expect_directive(
            &tokens,
            &mut index,
            TokenKind::Symbol(';'),
            source.len(),
            path,
        )?;
        end = tokens[index - 1].span.end;
        parts.push(PartDirective {
            uri,
            span: Span { start, end },
        });
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
    Ok(UnitDirectives {
        library,
        part_of,
        imports: directives,
        parts,
        end,
    })
}

/// Lê identificador pontuado de library ou part of, rejeitando palavras reservadas.
fn dotted_name(
    tokens: &[Token<'_>],
    index: &mut usize,
    end: usize,
    path: &Path,
) -> Result<String, GraphError> {
    let mut name = String::new();
    loop {
        let Some(TokenKind::Word(part)) = tokens.get(*index).map(|token| token.kind) else {
            return Err(error_at(
                path,
                Some(token_span(tokens, *index, end)),
                "diretiva exige nome de biblioteca pontuado".into(),
            ));
        };
        if reserved_combinator(part) {
            return Err(error_at(
                path,
                Some(tokens[*index].span),
                "identificador inválido no nome da biblioteca".into(),
            ));
        }
        name.push_str(part);
        *index += 1;
        if tokens.get(*index).map(|token| token.kind) != Some(TokenKind::Symbol('.')) {
            return Ok(name);
        }
        name.push('.');
        *index += 1;
    }
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

    /// Prefixos FFI são restritos ao perfil nativo e participam da identidade do grafo.
    #[test]
    fn ffi_import_prefix_requires_native_profile() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'dart:ffi' as ffi; void main(){}");
        assert!(load(&entry).is_err());
        assert!(load_with_environment(&entry, &CompilationEnvironment::wasm()).is_err());
        let graph = load_with_environment(&entry, &CompilationEnvironment::native()).unwrap();
        assert_eq!(graph.units[0].imports[0].prefix.as_deref(), Some("ffi"));
        assert_eq!(graph.units[1].path, PathBuf::from("dart:ffi"));
        for source in [
            "export 'dart:ffi';",
            "import 'dart:ffi' show Native;",
            "import 'dart:core' as core;",
            "import 'file.dart' as local;",
            "import 'dart:ffi' as class;",
        ] {
            fixture.write("main.dart", source);
            assert!(
                load_with_environment(&entry, &CompilationEnvironment::native()).is_err(),
                "{source}"
            );
        }
    }

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
            "part 'a.dart'",
            "part of;",
            "library a",
            "library ;",
            "void main() {} part 'a.dart';",
            "import 'a.dart'; part of 'b.dart';",
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

    /// A parte entra no grafo ligada ao pai, por URI ou por nome de biblioteca.
    #[test]
    fn parts_join_the_parent_library() {
        let fixture = Fixture::new();
        let entry = fixture.write(
            "main.dart",
            "library app.exemplo; import 'outra.dart'; part 'sub/p.dart'; void main(){}",
        );
        fixture.write("outra.dart", "int outro()=>1;");
        fixture.write("sub/p.dart", "part of app.exemplo; int ajuda()=>2;");
        let graph = load(&entry).unwrap();
        assert_eq!(graph.units.len(), 3);
        assert_eq!(graph.units[0].parts[0].uri, "sub/p.dart");
        assert_eq!(graph.units[0].parts[0].target, 2);
        let span = graph.units[0].parts[0].span;
        assert_eq!(
            &graph.units[0].source[span.start..span.end],
            "part 'sub/p.dart';"
        );
        assert_eq!(graph.units[2].part_of, Some(0));
        assert!(graph.units[2].parts.is_empty());
        assert_eq!(graph.units[2].directives_end, "part of app.exemplo;".len());
        assert_eq!(graph.units[1].part_of, None);
        fixture.write("sub/p.dart", "part of '../main.dart'; int ajuda()=>2;");
        let graph = load(&entry).unwrap();
        assert_eq!(graph.units[2].part_of, Some(0));
        assert_eq!(graph, load(&entry).unwrap());
    }

    /// Reivindicações duplicadas, ciclos e destinos divergentes têm diagnóstico próprio.
    #[test]
    fn part_claims_are_exclusive_and_checked() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "part 'p.dart'; void main(){}");
        fixture.write("p.dart", "part of 'main.dart';");
        assert!(load(&entry).is_ok());
        for (parent, part, needle) in [
            (
                "part 'p.dart'; part 'p.dart'; void main(){}",
                "part of 'main.dart';",
                "part duplicado",
            ),
            (
                "import 'p.dart'; part 'p.dart'; void main(){}",
                "part of 'main.dart';",
                "já carregado como biblioteca",
            ),
            (
                "part 'main.dart'; void main(){}",
                "part of 'main.dart';",
                "a si mesma como parte",
            ),
            (
                "part 'p.dart'; void main(){}",
                "int ajuda()=>1;",
                "exige part of",
            ),
            (
                "part 'p.dart'; void main(){}",
                "part of 'outro.dart';",
                "em vez de",
            ),
            (
                "library app; part 'p.dart'; void main(){}",
                "part of outro.nome;",
                "não corresponde",
            ),
            (
                "part 'p.dart'; void main(){}",
                "part of app;",
                "não corresponde",
            ),
            (
                "part 'p.dart'; void main(){}",
                "part of 'main.dart'; import 'outro.dart';",
                "não pode declarar import",
            ),
            (
                "part 'p.dart'; void main(){}",
                "part of 'main.dart'; export 'outro.dart';",
                "não pode declarar import",
            ),
            (
                "part 'p.dart'; void main(){}",
                "part of 'main.dart'; part 'outro.dart';",
                "não pode declarar part",
            ),
            (
                "part 'p.dart'; void main(){}",
                "library p; int ajuda()=>1;",
                "não pode declarar library",
            ),
            (
                "part 'p.dart'; void main(){}",
                "int ajuda()=>1; part of 'main.dart';",
                "fora do prefixo",
            ),
            (
                "part 'p.dart'; void main(){}",
                "library p; part of 'main.dart';",
                "primeira diretiva",
            ),
            (
                "import 'outro.dart'; void main(){}",
                "int ajuda()=>1;",
                "declare part",
            ),
        ] {
            fixture.write("main.dart", parent);
            fixture.write("p.dart", part);
            fixture.write(
                "outro.dart",
                if needle == "declare part" {
                    "part of 'main.dart';"
                } else {
                    "int outro()=>3;"
                },
            );
            let error = load(&entry).unwrap_err();
            assert!(
                error.message.contains(needle),
                "{parent} | {part} -> {error}"
            );
            let span = error.span.expect("diagnóstico de part tem posição");
            let owner = std::fs::read_to_string(&error.path).unwrap();
            assert!(span.start <= span.end && span.end <= owner.len(), "{error}");
        }
        let entry = fixture.write("main.dart", "part of 'p.dart'; void main(){}");
        assert!(load(&entry).unwrap_err().message.contains("declare part"));
    }

    /// Duas bibliotecas distintas não podem reivindicar o mesmo arquivo.
    #[test]
    fn part_cannot_belong_to_two_libraries() {
        let fixture = Fixture::new();
        let entry = fixture.write(
            "main.dart",
            "import 'outra.dart'; part 'p.dart'; void main(){}",
        );
        fixture.write("outra.dart", "part 'p.dart';");
        fixture.write("p.dart", "part of 'main.dart';");
        let error = load(&entry).unwrap_err();
        assert_eq!(
            error.path,
            std::fs::canonicalize(fixture.0.join("outra.dart")).unwrap()
        );
        assert!(error.message.contains("parte de outra biblioteca"));
        fixture.write("outra.dart", "import 'p.dart';");
        assert!(
            load(&entry)
                .unwrap_err()
                .message
                .contains("não pode ser importada")
        );
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
