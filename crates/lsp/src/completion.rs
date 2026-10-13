//! `textDocument/completion`: membros pelo tipo estático do receptor, nomes
//! do escopo, palavras-chave e argumentos nomeados.
//!
//! O ponto de digitação quase nunca analisa: `a.` sem nome, `a.ca` sem `;`.
//! O parser recupera por comando (pula até o `;` ou a `}` do bloco), então o
//! comando incompleto some da árvore. Antes de analisar, o nome sob o cursor
//! é trocado por um identificador sentinela e, se preciso, um fecho curto
//! (`;`, `);` …) é acrescentado depois dele; vale a variante que põe o
//! sentinela numa expressão com o menor número de diagnósticos. O programa é
//! então carregado com esse texto e os corpos da biblioteca são inferidos
//! pela inferência comum (`crates/types`): o tipo do receptor é o
//! `get_type` do alvo, e o escopo léxico é capturado pela sonda da inferência
//! no identificador sentinela.
//!
//! Em posição de tipo (`Str▮ x`, `List<▮>`, `void f(p.▮ a)`), só tipos:
//! classes, mixins, enums, extension types, typedefs, parâmetros de tipo em
//! escopo, prefixos e `dynamic`/`void` — nunca valores.
//!
//! Nomes públicos de bibliotecas ainda não importadas (SDK e projeto,
//! pelos índices de [`crate::index`]) entram com a edição que acrescenta o
//! `import` (`additionalTextEdits`), como a importação automática do Dart;
//! não em partes (o `import` iria para outro arquivo).
//!
//! O filtro é o `FuzzyMatcher` do Dart ([`crate::fuzzy_matcher`]): prefixo, contém,
//! iniciais de palavras e subsequência que abre como o nome. A ordem é por
//! relevância e estável: o que começa com o digitado vem antes do que só
//! casa por aproximação; depois o grupo (argumentos nomeados, locais,
//! membros da classe — os herdados de `Object` por último —, declarações da
//! biblioteca, importados, prefixos, não importados, palavras-chave) e o
//! nome.

use crate::DocumentStore;
use crate::query::Query;
use crate::index::{ProjectIndex, SdkIndex};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, LibraryId,
};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::LibraryFeatures;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

mod target;
mod context;
mod isp;
mod isp_declarations;
mod isp_not_imported;
mod isp_helpers;
mod isp_keywords;
mod isp_visits;

/// Identificador que ocupa o lugar do nome sob o cursor na análise.
const SENTINEL: &str = "dartforge__completar";

/// Fechos tentados depois do sentinela, na ordem de preferência.
const CLOSURES: &[&str] = &["", ";", ")", ");", "))", "));", ")));", "]", "];", "});"];

/// `CompletionItemKind` do LSP.
pub(crate) mod kind {
    pub const METHOD: u32 = 2;
    pub const FUNCTION: u32 = 3;
    pub const CONSTRUCTOR: u32 = 4;
    pub const FIELD: u32 = 5;
    pub const VARIABLE: u32 = 6;
    pub const CLASS: u32 = 7;
    pub const MODULE: u32 = 9;
    pub const PROPERTY: u32 = 10;
    pub const ENUM: u32 = 13;
    pub const WORD_KEY: u32 = 14;
    pub const ENUM_MEMBER: u32 = 20;
    pub const TYPE_PARAMETER: u32 = 25;
}

/// Como completar uma chamada, quando o cliente aceita snippets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// Os parâmetros obrigatórios, na ordem (`x`, e `name: ` para nomeados):
    /// viram os marcadores `${1:x}` do snippet.
    Parameters(Vec<String>),
    /// Função com parâmetros de nomes desconhecidos (índice de nomes): o
    /// cursor fica entre os parênteses.
    Unknown,
}

/// Um `import` a acrescentar junto com o item (importação automática).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoImport {
    /// A URI importada (`dart:math`, `util.dart`).
    pub uri: String,
    /// Onde a diretiva entra no documento (bytes) e o texto dela.
    pub span: Span,
    pub text: String,
}

/// Um item oferecido ao editor.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    /// O que a lista mostra (`met(…)`, `name: `).
    pub label: String,
    /// `CompletionItemKind`.
    pub kind: u32,
    /// Tipo ou assinatura.
    pub detail: Option<String>,
    /// Texto que substitui o prefixo digitado.
    pub insert: String,
    /// Forma de chamada, para funções, métodos e construtores.
    pub call: Option<Call>,
    /// Arquivo e início da declaração, para a documentação do
    /// `completionItem/resolve`.
    pub origin: Option<(PathBuf, usize)>,
    /// A diretiva que o item acrescenta (nome ainda não importado).
    pub import: Option<AutoImport>,
    /// Grupo de ordenação (menor vem antes).
    group: u8,
    /// Qualidade do casamento com o digitado (0: prefixo; 1: aproximado).
    quality: u8,
    /// O que entra no cálculo da relevância (`crate::relevance`).
    rel: crate::relevance::Rel,
    /// Classe do item (elemento de topo), para o tipo de contexto.
    class: Option<ClassId>,
    /// A relevância calculada (`0..1000`).
    relevance: i32,
    /// `sortText` = `9999 − relevância` (`MAP:59`, `:735-736`).
    pub sort_text: String,
    /// O score do `FuzzyMatcher` do handler (`fuzzy.suggestionScore`), para
    /// o truncamento.
    pub score: f64,
    /// `selectionOffset`/`selectionLength` (unidades UTF-16 do texto
    /// inserido), quando a sugestão põe o cursor antes do fim.
    pub selection: Option<(usize, usize)>,
    /// `displayText` (o rótulo, quando difere do texto inserido).
    pub display: Option<String>,
    /// O item veio do `InScopeCompletionPass` (a ordem e o `matcherScore`
    /// são os dele).
    pub(crate) of_pass: bool,
    /// `CompletionSuggestionKind.IDENTIFIER` num executável (tear-off,
    /// redirecionamento): o rótulo mostra os parâmetros, a inserção não põe
    /// parênteses.
    pub(crate) identifier: bool,
    /// O texto que o filtro do handler casa (`textToMatchOverride`:
    /// `override_x`, `setState`), quando difere do texto inserido.
    pub(crate) match_: Option<String>,
    /// `replacementLength: 0`: a inserção não substitui o prefixo.
    pub(crate) replace_empty: bool,
}

/// Resultado do completar: o intervalo do prefixo (bytes) e os itens em ordem.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completion {
    /// O `replacementOffset` (bytes).
    pub start: usize,
    /// O fim do `insert` (`replacementOffset + min(offset -
    /// replacementOffset, replacementLength)`).
    pub end: usize,
    /// O fim do `replacementRange`, quando o passe o calculou (senão o
    /// handler estende a palavra).
    pub replacement_end: Option<usize>,
    pub items: Vec<CompletionItem>,
    /// A lista foi cortada (nomes não importados acima do teto).
    pub incomplete: bool,
}

/// Onde o sentinela caiu na análise.
#[derive(Clone, Copy)]
enum Sentinel {
    /// Numa expressão; com o alvo quando é `target.▮`.
    Expr(ExprId, Option<ExprId>),
    /// Num nome de tipo; com o prefixo quando é `p.▮`, e se o tipo abre a
    /// declaração (de topo, membro ou comando), onde palavras-chave também
    /// cabem.
    Type {
        prefix: Option<SymbolId>,
        leader: Option<&'static [&'static str]>,
    },
}

/// O que o completar consulta além do programa.
pub(crate) struct Indices<'a> {
    pub sdk: &'a SdkIndex,
    pub project: &'a mut ProjectIndex,
    /// Os resumos das bibliotecas conhecidas (`FileStateFilter` e
    /// `exportNamespace`) dos não importados.
    pub known: &'a mut crate::known_libraries::LibraryIndex,
}

mod group {
    pub const NAMED: u8 = 0;
    pub const LOCAL: u8 = 1;
    pub const MEMBER: u8 = 2;
    /// Membros herdados de `Object` (`toString`, `hashCode`…).
    pub const OBJECT_MEMBER: u8 = 3;
    pub const LIBRARY: u8 = 4;
    pub const IMPORTED: u8 = 5;
    pub const PREFIX: u8 = 6;
    pub const NOT_IMPORTED: u8 = 7;
    pub const WORD: u8 = 8;
}

/// Teto de itens de bibliotecas não importadas numa resposta; acima dele a
/// lista sai com `isIncomplete` e o editor pede de novo a cada tecla.

const EXPRESSION_WORDS: &[&str] = &["const", "false", "new", "null", "true"];
const COMMAND_WORDS: &[&str] = &[
    "assert", "break", "const", "continue", "do", "false", "final", "for", "if", "late", "new",
    "null", "return", "switch", "throw", "true", "try", "var", "void", "while",
];
const TOP_WORDS: &[&str] = &[
    "abstract",
    "base",
    "class",
    "const",
    "enum",
    "export",
    "extension",
    "external",
    "final",
    "import",
    "interface",
    "late",
    "library",
    "mixin",
    "part",
    "sealed",
    "typedef",
    "var",
    "void",
];
const MEMBER_WORDS: &[&str] = &[
    "abstract",
    "const",
    "covariant",
    "external",
    "factory",
    "final",
    "get",
    "late",
    "operator",
    "set",
    "static",
    "var",
    "void",
];

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Completa na posição `offset` (bytes) de `text`, o conteúdo vigente de `uri`.
///
/// `None` quando o arquivo não pode ser carregado (URI que não é de
/// arquivo). Dentro de comentário, de texto de string ou de número, a lista
/// é vazia.
/// O `maxSuggestions` (`dart.maxCompletionItems`, padrão
/// `defaultMaxCompletions = 2000`).
pub(crate) const MAX_PATTERN: usize = 2000;

/// `SuggestionCollector.addSuggestion` (`SC:45-81`): inserção estável por
/// `matcherScore` decrescente e a poda acima de `max`.
fn collect(list: &mut Vec<(f64, CompletionItem)>, score: f64, item: CompletionItem, max: usize) {
    let mut position = 0;
    for k in (0..list.len()).rev() {
        if list[k].0 >= score {
            position = k + 1;
            break;
        }
    }
    list.insert(position, (score, item));
    if list.len() > max {
        let min = list[max].0;
        while list.len() > max && list.last().is_some_and(|(s, _)| *s < min) {
            list.pop();
        }
    }
}

/// O texto pontuado (`displayName`): o rótulo de inserção sem o `: ` de um
/// argumento nomeado.
fn scored_text(i: &CompletionItem) -> &str {
    match &i.match_ {
        Some(c) => c,
        None => i.insert.trim_end_matches([':', ' ']),
    }
}

pub(crate) fn completion(
    sdk: &SdkLayout,
    indices: Indices<'_>,
    documents: &DocumentStore,
    uri: &str,
    text: &str,
    offset: usize,
    features: LibraryFeatures,
    max: usize,
    not_imported: bool,
    retained: Option<&mut crate::project::Project>,
) -> Option<Completion> {
    let bytes = text.as_bytes();
    let offset = offset.min(text.len());
    let mut start = offset;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }
    let empty = Completion {
        start: offset,
        end: offset,
        replacement_end: None,
        items: Vec::new(),
        incomplete: false,
    };
    // O texto analisado troca a palavra sob o cursor pelo sentinela (só para
    // a semântica: o contexto é o do texto real, `completion/isp.rs`).
    let typed = &text[start..offset];
    let prepared = prepare(text, start, end, features);
    let analyzed_text = prepared.as_deref().unwrap_or(text);
    let delta = analyzed_text.len() as isize - text.len() as isize;
    let request = Request { sdk, indices, documents, text, start, end, offset, typed, speculative: prepared.is_some(), max, not_imported, delta };
    // O caminho incremental (docs/LSP-ESPECIFICACAO.md §16.7): o estado
    // retido é o do texto do documento; o texto com o sentinela troca a
    // unidade no lugar, só o corpo do sentinela é inferido (com a sonda), e a
    // troca é desfeita depois da resposta. Fora de um corpo, ou com a
    // assinatura, as diretivas ou a forma mudadas, a carga completa.
    if let Some(project) = retained
        && let Some(unit) = project.uri_unit(uri)
        && project.program().unit(unit).source == text
        && project.libraries.contains(&project.program().unit(unit).library)
    {
        let query = &mut project.query;
        match prepared.as_deref() {
            None => return respond(query, unit, request, empty),
            Some(speculative) => {
                if let Some(swap) = query.swap_unit(unit, speculative, crate::query::SwapMode::Speculative { offset: start }) {
                    let response = respond(query, unit, request, empty);
                    query.undo(swap);
                    return response;
                }
            }
        }
    }
    let (program, names, unit) =
        crate::semantic::load(sdk, uri, analyzed_text, Some(documents))?;
    let library = program.unit(unit).library;
    let probe = prepared.as_ref().map(|_| (unit, start));
    let mut query = Query::infer(program, names, &[library], false, probe);
    respond(&mut query, unit, request, empty)
}

/// O pedido de completar, já com o intervalo da palavra.
struct Request<'a, 'i> {
    sdk: &'a SdkLayout,
    indices: Indices<'i>,
    documents: &'a DocumentStore,
    text: &'a str,
    start: usize,
    end: usize,
    offset: usize,
    typed: &'a str,
    /// O texto analisado tem o sentinela.
    speculative: bool,
    max: usize,
    /// Sugerir os não importados (`suggestFromUnimportedLibraries` com
    /// `workspace/applyEdit`).
    not_imported: bool,
    /// O deslocamento dos offsets depois da palavra no texto analisado.
    delta: isize,
}

/// As sugestões sobre a consulta pronta (a retida, com a unidade trocada
/// pelo texto especulativo, ou a de uma carga): o `InScopeCompletionPass`
/// sobre o texto real (`completion/isp.rs`), o `NotImportedCompletionPass`,
/// a relevância de cada candidato, o coletor (`matcherScore`, poda), o
/// mapa de sugestões e o filtro e o truncamento do handler.
fn respond(query: &mut Query, unit: dartforge_elements::model::UnitId, request: Request<'_, '_>, empty: Completion) -> Option<Completion> {
    let Request { sdk, indices, documents, text, start, end, offset, typed: _, speculative, max, not_imported, delta } = request;
    let features = query.program.unit(unit).features;
    let before_3 = features.versao() < dartforge_frontend::LanguageVersion::new(3, 0);
    let swap = speculative.then_some((start, end, SENTINEL.len(), delta));
    // Numa parte, o `import` iria para a biblioteca dona (outro arquivo).
    let part = query.program.unit(unit).unit.directives.iter().any(|d| matches!(d.kind, ast::DirectiveKind::PartOf { .. }));
    // As bibliotecas candidatas dos não importados (`FileStateFilter`, menos
    // a do pedido e as importadas sem combinadores pela unidade definidora,
    // `_ImportSummary`), com a URI do `import`.
    let filter = candidates_filter(query, unit);
    let Indices { sdk: _, project: _, known } = indices;
    let candidates = |c: &mut crate::known_libraries::LibraryIndex| -> Vec<(PathBuf, String)> {
        let Some((file, own, imported)) = &filter else { return Vec::new() };
        let root = crate::project::project_root(file);
        let package = crate::index::package_name(&root);
        let key = |x: &std::path::Path| dartforge_elements::gerado::chave(x);
        let mut v = Vec::new();
        for candidate in crate::known_libraries::candidates(file, Some(sdk), c, documents) {
            let k = key(&candidate.path);
            if own.contains(&k) || imported.contains(&k) {
                continue;
            }
            let uri = if candidate.sdk || candidate.uri.starts_with("package:") {
                candidate.uri.clone()
            } else {
                match crate::index::import_uri(file, &candidate.path, &root, package.as_deref()) {
                    Some(u) => u,
                    None => continue,
                }
            };
            v.push((candidate.path, uri));
        }
        v
    };
    let r = {
        let mut list = || candidates(known);
        let not_imported_: Option<&mut dyn FnMut() -> Vec<(PathBuf, String)>> = if part || !not_imported { None } else { Some(&mut list) };
        isp::run(query, unit, text, features, offset, swap, before_3, not_imported_)
    };
    if r.comment {
        return Some(empty);
    }
    let library = query.program.unit(unit).library;
    // O sentinela é do texto analisado, não do usuário.
    let mut arrival: Vec<(f64, CompletionItem)> = r.items.into_iter().filter(|(_, i)| !i.insert.contains(SENTINEL)).collect();
    // `StaticMembersOperation` sobre as bibliotecas que a consulta não
    // carregou: os nomes do resumo do `exportNamespace`.
    if r.operations.iter().any(|o| matches!(o, isp::Operation::StaticMembers)) && !r.summarized.is_empty() {
        let only_types = r.flags.as_ref().is_some_and(|f| f.ty);
        let mut collector = Collector { items: Vec::new(), library, receiver: None };
        let mut names = Interner::new();
        let real = dartforge_frontend::parser::parse_com(text, &mut names, features);
        if let Some((file, _, _)) = &filter {
            collector.not_imported_summary(text, &real.unit, only_types, sdk, file, known, documents, &r.summarized, &r.visible);
        }
        let mut matcher = (!r.matcher_prefix.is_empty()).then(|| crate::fuzzy_matcher::Matcher::new(&r.matcher_prefix, crate::fuzzy_matcher::Style::Text));
        for i in collector.items {
            let s = match matcher.as_mut() {
                Some(c) => c.score(i.insert.as_str()),
                None => 0.0,
            };
            if s != -1.0 {
                arrival.push((s, i));
            }
        }
    }
    // A relevância (`relevanceComputer.computeRelevance`): o local gravado
    // pelo passe, o tipo de contexto do pedido e o `preferConstants`.
    let local = r.local.clone();
    let expected = r.context_type;
    let prefer_constants = r.prefers_constants || r.in_constant_context;
    let type_bool = query.core.bool_;
    let null_type = query.core.null;
    for (_, i) in &mut arrival {
        if i.rel.ty.is_none() {
            i.rel.ty = match i.rel.word {
                Some("true" | "false") => Some(type_bool),
                Some("null") => Some(null_type),
                _ => None,
            };
        }
        if i.rel.ty.is_none()
            && let Some(c) = i.class
        {
            i.rel.ty = class_type(query, c);
        }
        let context = match (expected, i.rel.ty) {
            (Some(e), Some(t)) => context_feature(query, e, t),
            _ => 0.0,
        };
        i.relevance = crate::relevance::relevance(&i.rel, local.as_deref(), context, prefer_constants);
        i.sort_text = (9999 - i.relevance).to_string();
    }
    // O coletor: inserção estável por `matcherScore` e a poda.
    let mut list: Vec<(f64, CompletionItem)> = Vec::new();
    for (s, i) in arrival {
        collect(&mut list, s, i, max);
    }
    // O `_suggestionMap` (`SB:989-1026`): a chave é o texto (o construtor
    // com `()`, o não importado com `::uri`); o último vence, na posição do
    // primeiro; o construtor cujo nome de classe já está no mapa por um
    // elemento que não é classe fica de fora.
    let mut order: Vec<CompletionItem> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (_, i) in list {
        if i.kind == kind::CONSTRUCTOR
            && let Some(c) = i.class
        {
            let class_name = query.name(query.program.class(c).name).to_string();
            if let Some(&k) = index.get(&class_name) {
                let existing = &order[k];
                let is_class = existing.class.is_some_and(|x| {
                    let cl = query.program.class(x);
                    matches!(cl.kind, ClassKind::Class | ClassKind::MixinApplication)
                }) && existing.kind == kind::CLASS;
                if !is_class {
                    continue;
                }
            }
        }
        let mut key = i.insert.clone();
        if i.kind == kind::CONSTRUCTOR {
            key.push_str("()");
        }
        if let Some(imp) = &i.import
            && i.rel.not_imported
        {
            key.push_str("::");
            key.push_str(&imp.uri);
        }
        match index.get(&key) {
            Some(&k) => order[k] = i,
            None => {
                index.insert(key, order.len());
                order.push(i);
            }
        }
    }
    // O handler: `fuzzy.suggestionScore(item) > 0` pelo prefixo do pedido
    // (vazio → 1.0).
    let mut fuzzy = crate::fuzzy_matcher::Matcher::new(&r.request_prefix, crate::fuzzy_matcher::Style::Text);
    for i in &mut order {
        i.score = fuzzy.score(scored_text(i));
    }
    order.retain(|i| i.score > 0.0);
    let mut incomplete = r.incomplete;
    // `_truncateResults` acima do máximo.
    if order.len() > max {
        let prefix = r.request_prefix.to_lowercase();
        order.sort_by(|a, b| {
            if a.score != b.score {
                return b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal);
            }
            if a.sort_text == b.sort_text {
                return a.label.len().cmp(&b.label.len());
            }
            a.sort_text.cmp(&b.sort_text)
        });
        let mut k = 0usize;
        order.retain(|i| {
            let keep = k < max || (!prefix.is_empty() && scored_text(i).to_lowercase() == prefix);
            k += 1;
            keep
        });
        incomplete = true;
    }
    // `replacementRange` e o `insert` do handler (`min(offset -
    // replacementOffset, replacementLength)`).
    let (ri, rl) = r.range;
    let insertion = ri + offset.saturating_sub(ri).min(rl);
    Some(Completion { start: ri, end: insertion, replacement_end: Some(ri + rl), items: order, incomplete })
}

/// O que o filtro das candidatas dos não importados precisa: o arquivo do
/// pedido, as unidades da própria biblioteca e as bibliotecas importadas
/// sem combinadores pela unidade definidora (`_ImportSummary`).
#[allow(clippy::type_complexity)]
fn candidates_filter(query: &Query, unit: dartforge_elements::model::UnitId) -> Option<(PathBuf, HashSet<PathBuf>, HashSet<PathBuf>)> {
    let program = &query.program;
    let lib = program.library(program.unit(unit).library);
    let file = program.unit(unit).path.clone()?;
    let defining = lib.units.first().copied();
    let key = |c: &std::path::Path| dartforge_elements::gerado::chave(c);
    let imported: HashSet<PathBuf> = lib
        .imports
        .iter()
        .filter(|i| Some(i.unit) == defining && i.combinators.is_empty())
        .filter_map(|i| program.library(i.library).units.first())
        .filter_map(|u| program.unit(*u).path.as_deref().map(key))
        .collect();
    let own: HashSet<PathBuf> = lib.units.iter().filter_map(|u| program.unit(*u).path.as_deref().map(key)).collect();
    Some((file, own, imported))
}

/// Texto com o nome sob o cursor trocado pelo sentinela (e o fecho que o
/// faz analisar); `None` quando nenhuma variante põe o sentinela numa
/// expressão ou num nome de tipo. Vale a variante com menos diagnósticos
/// (a primeira, entre iguais).
fn prepare(text: &str, start: usize, end: usize, features: LibraryFeatures) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for closure in CLOSURES {
        let candidate = format!("{}{SENTINEL}{closure}{}", &text[..start], &text[end..]);
        let mut names = Interner::new();
        let analyzed = dartforge_frontend::parser::parse_com(&candidate, &mut names, features);
        if find_sentinel(&analyzed.ast, start).is_none() {
            continue;
        }
        // Sem fecho, o texto tem a árvore do real (o parser insere os
        // fechos sintéticos como o fasta): é a escolha sempre que o
        // sentinela cai numa expressão ou num tipo.
        if closure.is_empty() {
            return Some(candidate);
        }
        let errors = analyzed.diagnostics.len();
        if best.as_ref().is_none_or(|(e, _)| errors < *e) {
            let zero = errors == 0;
            best = Some((errors, candidate));
            if zero {
                break;
            }
        }
    }
    best.map(|(_, t)| t)
}

/// Onde está o sentinela em `start`: numa expressão (com o alvo, se é
/// acesso a membro) ou num nome de tipo.
fn find_sentinel(ast: &ast::Ast, start: usize) -> Option<Sentinel> {
    let eh = |n: &ast::Name| n.span.start == start && n.span.end == start + SENTINEL.len();
    let expr = ast
        .exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Identifier(n) if eh(n) => Some(Sentinel::Expr(ExprId(i as u32), None)),
            ExprKind::Property { target, name, .. } if eh(name) => {
                Some(Sentinel::Expr(ExprId(i as u32), Some(*target)))
            }
            _ => None,
        });
    if expr.is_some() {
        return expr;
    }
    let (ty, name_) = ast.types.iter().find_map(|t| match &t.kind {
        ast::TypeKind::Named { name, .. } if name.last().is_some_and(eh) => Some((t, name)),
        _ => None,
    })?;
    let prefix = (name_.len() == 2).then(|| name_[0].sym);
    // O tipo abre uma declaração de topo, de membro ou um comando: ali as
    // palavras-chave de declaração também cabem.
    let opens = |s: dartforge_diagnostics::Span| s.start == ty.span.start;
    let leader: Option<&'static [&'static str]> = if prefix.is_some() {
        None
    } else if ast.decls.iter().any(|d| opens(d.span)) {
        Some(TOP_WORDS)
    } else if ast.members.iter().any(|m| opens(m.span)) {
        Some(MEMBER_WORDS)
    } else if ast.stmts.iter().any(|s| opens(s.span)) {
        Some(COMMAND_WORDS)
    } else {
        None
    };
    Some(Sentinel::Type { prefix, leader })
}

struct Collector {
    items: Vec<CompletionItem>,
    library: LibraryId,
    /// Classe do receptor enquanto se coletam membros de instância (a
    /// distância de herança de cada membro).
    receiver: Option<ClassId>,
}

impl Collector {
    fn push(
        &mut self,
        group: u8,
        kind: u32,
        label: String,
        insert: String,
        detail: Option<String>,
    ) -> &mut CompletionItem {
        self.items.push(CompletionItem {
            label,
            kind,
            detail,
            insert,
            call: None,
            origin: None,
            import: None,
            group,
            quality: 0,
            rel: crate::relevance::Rel::default(),
            class: None,
            relevance: 0,
            sort_text: String::new(),
            score: 0.0,
            selection: None,
            display: None,
            of_pass: false,
            identifier: false,
            match_: None,
            replace_empty: false,
        });
        self.items.last_mut().expect("item recém-empurrado")
    }

    /// Preenche a relevância do último item a partir da função `f` (com o
    /// tipo `ty` visto pelo receptor).
    fn function_rel(&mut self, query: &Query, f: FunctionElementId, ty: TypeId) {
        use crate::relevance::Kind;
        let fe = query.program.function(f);
        let member = fe.class.is_some() || fe.extension.is_some();
        let name = query.name(fe.name).to_string();
        let enum_constant = fe.variable.is_some_and(|v| {
            let ve = query.program.variable(v);
            ve.class.is_some_and(|c| query.program.class(c).enum_constants.contains(&v))
        });
        let (kind, item_type) = match fe.kind {
            _ if enum_constant => (Kind::NoTable, Some(ty)),
            FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                (if member { Kind::Field } else { Kind::TopLevelVariable }, Some(ty))
            }
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => (Kind::Constructor, return_(query, ty)),
            FunctionKind::Function | FunctionKind::Operator => {
                (if member { Kind::Method } else { Kind::Function }, return_(query, ty))
            }
        };
        let distance = match (self.receiver, fe.class) {
            (Some(r), Some(c)) if member => inheritance_distance(query, r, c).map(|d| d as u32),
            _ => None,
        };
        // `isConstantFeature`: construtor `const`; acessor sintético de
        // variável estática (ou de topo) `const`.
        let constant = match fe.kind {
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => fe.const_,
            FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| {
                let ve = query.program.variable(v);
                ve.const_ && (ve.static_ || (ve.class.is_none() && ve.extension.is_none()))
            }),
            _ => false,
        };
        let deprecated = function_deprecated(query, f);
        if let Some(item) = self.items.last_mut() {
            item.rel.kind = Some(kind);
            item.rel.ty = item_type;
            item.rel.distance = distance;
            item.rel.starts_with_dollar = name.starts_with('$');
            item.rel.no_such_method = name == "noSuchMethod";
            item.rel.constant = constant;
            item.rel.deprecated = deprecated;
        }
    }

    /// Nome privado de outra biblioteca: invisível aqui.
    fn invisible(&self, query: &Query, name: &str, owning: LibraryId) -> bool {
        let _ = query;
        name.starts_with('_') && owning != self.library
    }

    /// Item de uma função (método, getter, acessor) com o tipo `ty` já
    /// visto pelo receptor.
    fn function(&mut self, query: &Query, group: u8, f: FunctionElementId, ty: TypeId) {
        let fe = query.program.function(f);
        let name = query.name(fe.name).trim_end_matches('=').to_string();
        if name.is_empty()
            || !name.as_bytes()[0].is_ascii_alphabetic() && !name.starts_with(['_', '$'])
        {
            return;
        }
        if self.invisible(query, &name, fe.library) {
            return;
        }
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let enum_constant = fe.variable.is_some_and(|v| {
                    let ve = query.program.variable(v);
                    ve.class
                        .is_some_and(|c| query.program.class(c).enum_constants.contains(&v))
                });
                let kind = if enum_constant {
                    kind::ENUM_MEMBER
                } else if fe.class.is_some() || fe.extension.is_some() {
                    kind::FIELD
                } else {
                    kind::VARIABLE
                };
                let origin = query.origin(query.function_start(f));
                self.push(
                    group,
                    kind,
                    name.clone(),
                    name,
                    Some(query.format(ty)),
                )
                .origin = origin;
                self.function_rel(query, f, ty);
            }
            FunctionKind::Getter | FunctionKind::Setter => {
                let kind = if fe.class.is_some() || fe.extension.is_some() {
                    kind::PROPERTY
                } else {
                    kind::VARIABLE
                };
                let origin = query.origin(query.function_start(f));
                self.push(
                    group,
                    kind,
                    name.clone(),
                    name,
                    Some(query.format(ty)),
                )
                .origin = origin;
                self.function_rel(query, f, ty);
            }
            FunctionKind::Function
            | FunctionKind::Constructor
            | FunctionKind::SyntheticConstructor => {
                let kind = match fe.kind {
                    FunctionKind::Function if fe.class.is_some() || fe.extension.is_some() => {
                        kind::METHOD
                    }
                    FunctionKind::Function => kind::FUNCTION,
                    _ => kind::CONSTRUCTOR,
                };
                let without_parameters = matches!(query.table.get(ty), Type::Function { positional, optional, named, .. }
                    if positional.is_empty() && optional.is_empty() && named.is_empty());
                let label = if without_parameters {
                    format!("{name}()")
                } else {
                    format!("{name}(…)")
                };
                // Os obrigatórios viram marcadores do snippet: posicionais
                // pelo nome e nomeados como `name: `.
                let parameters: Vec<String> = query.outline.functions[f.0 as usize]
                    .parameters
                    .iter()
                    .filter_map(|p| {
                        let n = query.name(p.externo.or(p.name)?).to_string();
                        match p.kind {
                            ast::ParameterKind::Required => Some(n),
                            ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                            _ => None,
                        }
                    })
                    .collect();
                let origin = query.origin(query.function_start(f));
                let item = self.push(
                    group,
                    kind,
                    label,
                    name,
                    Some(query.function_detail(f, ty)),
                );
                item.call = Some(Call::Parameters(parameters));
                item.origin = origin;
                self.function_rel(query, f, ty);
            }
            FunctionKind::Operator => {}
        }
    }

    /// Item de um elemento de topo (da biblioteca ou importado).
    fn element(&mut self, query: &Query, group: u8, element: Element) {
        let program = &query.program;
        match element {
            Element::Class(c) => {
                let class = program.class(c);
                let name = query.name(class.name).to_string();
                if self.invisible(query, &name, class.library)
                    || class.kind == ClassKind::MixinApplication && class.decl.is_none()
                {
                    return;
                }
                let kind = if class.kind == ClassKind::Enum {
                    kind::ENUM
                } else {
                    kind::CLASS
                };
                let kind_rel = match class.decl.map(|d| &program.unit(d.unit).ast.decl(d.decl).kind) {
                    Some(ast::DeclKind::Enum(_)) => crate::relevance::Kind::Enum,
                    Some(ast::DeclKind::Mixin(_)) => crate::relevance::Kind::Mixin,
                    Some(ast::DeclKind::ExtensionType(_)) => crate::relevance::Kind::NoTable,
                    _ => crate::relevance::Kind::Class,
                };
                let origin = query.origin(query.element_start(element));
                let deprecated = element_deprecated(query, element);
                let item = self.push(group, kind, name.clone(), name, None);
                item.origin = origin;
                item.rel.kind = Some(kind_rel);
                item.rel.deprecated = deprecated;
                item.class = Some(c);
            }
            Element::Typedef(t) => {
                let name = query.name(program.typedef(t).name).to_string();
                if !self.invisible(query, &name, program.typedef(t).library) {
                    let origin = query.origin(query.element_start(element));
                    let target_function = matches!(query.table.get(query.outline.typedefs[t.0 as usize].target_type), Type::Function { .. });
                    let item = self.push(group, kind::CLASS, name.clone(), name, None);
                    item.origin = origin;
                    item.rel.kind = Some(if target_function {
                        crate::relevance::Kind::FunctionAlias
                    } else {
                        crate::relevance::Kind::NoTable
                    });
                }
            }
            Element::Extension(x) => {
                let Some(n) = program.extension(x).name else {
                    return;
                };
                let name = query.name(n).to_string();
                if !self.invisible(query, &name, program.extension(x).library) {
                    let origin = query.origin(query.element_start(element));
                    let item = self.push(group, kind::CLASS, name.clone(), name, None);
                    item.origin = origin;
                    item.rel.kind = Some(crate::relevance::Kind::NoTable);
                }
            }
            Element::Function(f) => {
                let data = &query.outline.functions[f.0 as usize];
                let ty = match program.function(f).kind {
                    FunctionKind::Getter | FunctionKind::ImplicitAccessor => data.return_type,
                    FunctionKind::Setter => data
                        .parameters
                        .first()
                        .map_or(query.core.dynamic_, |p| p.ty),
                    _ => data.signature,
                };
                let ty = match program
                    .function(f)
                    .variable
                    .and_then(|v| query.variable_type(v))
                {
                    Some(t) if program.function(f).kind == FunctionKind::ImplicitAccessor => t,
                    _ => ty,
                };
                self.function(query, group, f, ty);
            }
            Element::Variable(v) => {
                let ve = program.variable(v);
                let name = query.name(ve.name).to_string();
                if !self.invisible(query, &name, ve.library) {
                    let detail = query.variable_type(v).map(|t| query.format(t));
                    let origin = query.origin(query.variable_start(v));
                    let deprecated = element_deprecated(query, element);
                    let item = self.push(group, kind::VARIABLE, name.clone(), name, detail);
                    item.origin = origin;
                    item.rel.kind = Some(crate::relevance::Kind::TopLevelVariable);
                    item.rel.ty = query.variable_type(v);
                    item.rel.constant = ve.const_;
                    item.rel.deprecated = deprecated;
                }
            }
            Element::Prefix(_, p) => {
                let name = query.name(p).to_string();
                self.push(group::PREFIX, kind::MODULE, name.clone(), name, None)
                    .rel
                    .kind = Some(crate::relevance::Kind::Prefix);
            }
        }
    }

    /// A `StaticMembersOperation` do `NotImportedCompletionPass`
    /// (docs/LSP-ESPECIFICACAO.md §14.9) sobre as bibliotecas que a
    /// consulta não carregou: as declarações do `exportNamespace` pelo resumo
    /// de nomes (`_addExternalTopLevelDeclarations`: só tipos com
    /// `mustBeType`), cada uma com o `import` que a torna visível; o
    /// `VisibilityTracker` do passe esconde as de nome já visto. O resumo não
    /// tem os construtores nem os campos estáticos das classes.
    #[allow(clippy::too_many_arguments)]
    fn not_imported_summary(
        &mut self,
        text: &str,
        unit: &ast::CompilationUnit,
        only_types: bool,
        sdk: &SdkLayout,
        file: &std::path::Path,
        known: &mut crate::known_libraries::LibraryIndex,
        documents: &DocumentStore,
        candidates: &[(PathBuf, String)],
        visible: &HashSet<String>,
    ) {
        use crate::known_libraries::Kind as E;
        let resolver = crate::known_libraries::Resolver {
            sdk: Some(sdk),
            packages: dartforge_elements::config::PackageConfig::discover(file).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()),
        };
        for (path, uri) in candidates {
            for (name, origin, d) in known.exported(path, &resolver, documents) {
                let ty = crate::known_libraries::OF_TYPE.contains(&d.kind);
                if (only_types && !ty) || visible.contains(&name) {
                    continue;
                }
                let (lsp_kind, rel) = match d.kind {
                    E::Class | E::ExtensionType => (kind::CLASS, crate::relevance::Kind::Class),
                    E::Mixin => (kind::CLASS, crate::relevance::Kind::Mixin),
                    E::Enum => (kind::ENUM, crate::relevance::Kind::Enum),
                    E::FunctionAlias => (kind::CLASS, crate::relevance::Kind::FunctionAlias),
                    E::TypeAlias | E::Extension => (kind::CLASS, crate::relevance::Kind::NoTable),
                    E::Function => (kind::FUNCTION, crate::relevance::Kind::Function),
                    E::Variable => (kind::VARIABLE, crate::relevance::Kind::TopLevelVariable),
                };
                let function = d.kind == E::Function;
                let label = match (function, d.without_parameters) {
                    (true, true) => format!("{name}()"),
                    (true, false) => format!("{name}(…)"),
                    _ => name.clone(),
                };
                let (span, new) = crate::actions::insert_import(text, unit, uri);
                // A assinatura escrita do resumo é o `detail` (o `Auto
                // import from` vem no `completionItem/resolve`).
                let item = self.push(group::NOT_IMPORTED, lsp_kind, label, name.clone(), d.signature.as_deref().map(str::to_string));
                item.import = Some(AutoImport { uri: uri.clone(), span, text: new });
                item.origin = Some((origin, d.start));
                item.rel.not_imported = true;
                item.rel.kind = Some(rel);
                item.of_pass = true;
                if function {
                    item.call = Some(if d.without_parameters { Call::Parameters(Vec::new()) } else { Call::Unknown });
                }
            }
        }
    }

}

/// Os nomes das anotações (o último segmento) de uma lista de metadados.
fn annotation_names(query: &Query, ms: &[ast::Annotation]) -> Vec<String> {
    ms.iter().filter_map(|m| m.name.last().map(|n| query.name(n.sym).to_string())).collect()
}

/// `@deprecated`/`@Deprecated(...)` entre os metadados.
fn has_deprecated(query: &Query, ms: &[ast::Annotation]) -> bool {
    annotation_names(query, ms).iter().any(|n| n == "deprecated" || n == "Deprecated")
}

/// `hasDeprecated` de um elemento de topo.
fn element_deprecated(query: &Query, el: Element) -> bool {
    let p = &query.program;
    let decl = match el {
        Element::Class(c) => p.class(c).decl,
        Element::Extension(x) => Some(p.extension(x).decl),
        Element::Typedef(t) => Some(p.typedef(t).decl),
        Element::Function(f) => return function_deprecated(query, f),
        Element::Variable(v) => match p.variable(v).node {
            dartforge_elements::model::VariableRef::TopLevel { unit, decl, .. } => Some(dartforge_elements::model::DeclRef { unit, decl }),
            _ => None,
        },
        Element::Prefix(..) => None,
    };
    decl.is_some_and(|d| has_deprecated(query, &p.unit(d.unit).ast.decl(d.decl).metadata))
}

/// `hasOrInheritsDeprecated` de uma função, método, acessor ou construtor:
/// a anotação dela, ou a do membro de mesmo nome que ela sobrescreve.
fn function_deprecated(query: &Query, f: FunctionElementId) -> bool {
    let p = &query.program;
    let own = |f: FunctionElementId| -> bool {
        match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let a = &p.unit(unit).ast;
                a.members
                    .iter()
                    .find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x == function))
                    .map(|m| has_deprecated(query, &m.metadata))
                    .or_else(|| a.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(x) if x == function)).map(|d| has_deprecated(query, &d.metadata)))
                    .unwrap_or(false)
            }
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => has_deprecated(query, &p.unit(unit).ast.member(member).metadata),
            dartforge_elements::model::FunctionRef::None => match p.function(f).variable.map(|v| p.variable(v).node) {
                Some(dartforge_elements::model::VariableRef::Field { unit, member, .. }) => has_deprecated(query, &p.unit(unit).ast.member(member).metadata),
                Some(dartforge_elements::model::VariableRef::TopLevel { unit, decl, .. }) => has_deprecated(query, &p.unit(unit).ast.decl(decl).metadata),
                Some(dartforge_elements::model::VariableRef::EnumConstant { unit, decl, index }) => match &p.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Enum(e) => e.constants.get(index).is_some_and(|k| has_deprecated(query, &k.metadata)),
                    _ => false,
                },
                _ => false,
            },
        }
    };
    if own(f) {
        return true;
    }
    // O membro sobrescrito (`inheritsDeprecated`): o de mesmo nome nos
    // supertipos.
    let fe = p.function(f);
    let Some(c) = fe.class else { return false };
    if fe.static_ || matches!(fe.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
        return false;
    }
    let mut stack: Vec<ClassId> = Vec::new();
    let cl = p.class(c);
    stack.extend(cl.supertype_class);
    stack.extend(cl.mixin_classes.iter().copied());
    stack.extend(cl.interface_classes.iter().copied());
    stack.extend(cl.on_classes.iter().copied());
    let mut seen = HashSet::new();
    while let Some(x) = stack.pop() {
        if !seen.insert(x) {
            continue;
        }
        let s = p.class(x);
        if let Some(&g) = s.instance_members.get(&fe.name)
            && own(g)
        {
            return true;
        }
        stack.extend(s.supertype_class);
        stack.extend(s.mixin_classes.iter().copied());
        stack.extend(s.interface_classes.iter().copied());
        stack.extend(s.on_classes.iter().copied());
    }
    false
}

/// Tipo declarado de um membro, sem receptor (estáticos e extensões).
fn declared_type(query: &Query, f: FunctionElementId) -> TypeId {
    let fe = query.program.function(f);
    let data = &query.outline.functions[f.0 as usize];
    match fe.kind {
        FunctionKind::ImplicitAccessor => fe
            .variable
            .and_then(|v| query.variable_type(v))
            .unwrap_or(data.return_type),
        FunctionKind::Getter => data.return_type,
        FunctionKind::Setter => data
            .parameters
            .first()
            .map_or(query.core.dynamic_, |p| p.ty),
        _ => data.signature,
    }
}

/// Biblioteca que declara um elemento de topo.
fn element_library(query: &Query, element: Element) -> LibraryId {
    let p = &query.program;
    match element {
        Element::Class(c) => p.class(c).library,
        Element::Extension(x) => p.extension(x).library,
        Element::Typedef(t) => p.typedef(t).library,
        Element::Function(f) => p.function(f).library,
        Element::Variable(v) => p.variable(v).library,
        Element::Prefix(l, _) => l,
    }
}

/// O retorno de um tipo de função.
fn return_(query: &Query, ty: TypeId) -> Option<TypeId> {
    match query.table.get(ty) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }
}

/// Arestas de `sub` até `sup` na hierarquia (superclasse, mixins,
/// interfaces, `on`), como o `_inheritanceDistance` do Dart.
fn inheritance_distance(query: &Query, sub: ClassId, sup: ClassId) -> Option<usize> {
    let mut queue = std::collections::VecDeque::from([(sub, 0usize)]);
    let mut seen = HashSet::new();
    while let Some((c, d)) = queue.pop_front() {
        if c == sup {
            return Some(d);
        }
        if !seen.insert(c) {
            continue;
        }
        let cl = query.program.class(c);
        for s in cl.supertype_class.iter().chain(&cl.mixin_classes).chain(&cl.interface_classes).chain(&cl.on_classes) {
            queue.push_back((*s, d + 1));
        }
    }
    None
}

/// O tipo de interface de `c` com os argumentos `Never`, como o Dart
/// instancia uma classe sugerida (`instantiateInstanceElement`).
fn class_type(query: &mut Query, c: ClassId) -> Option<TypeId> {
    let n = query.outline.classes.get(c.0 as usize)?.type_params.len();
    let never = query.core.never;
    let args: Box<[TypeId]> = std::iter::repeat_n(never, n).collect();
    Some(query.table.intern(Type::Interface { class: c, args, nullable: false }))
}

/// `contextTypeFeature`: igual 1,0; subtipo 0,40; supertipo 0,02; sem
/// relação 0,13.
fn context_feature(query: &mut Query, expected: TypeId, ty: TypeId) -> f64 {
    if expected == ty {
        return 1.0;
    }
    let Query { table, outline, core, .. } = query;
    let mut env = dartforge_types::SubtypeEnv::new(table, &outline.hierarchy, core);
    if dartforge_types::is_subtype(ty, expected, &mut env) {
        0.40
    } else if dartforge_types::is_subtype(expected, ty, &mut env) {
        0.02
    } else {
        0.13
    }
}

