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
//! pelos índices de [`crate::indice`]) entram com a edição que acrescenta o
//! `import` (`additionalTextEdits`), como a importação automática do Dart;
//! não em partes (o `import` iria para outro arquivo).
//!
//! O filtro é aproximado ([`crate::aproximado`]): prefixo, contém,
//! iniciais de palavras e subsequência que abre como o nome. A ordem é por
//! relevância e estável: o que começa com o digitado vem antes do que só
//! casa por aproximação; depois o grupo (argumentos nomeados, locais,
//! membros da classe — os herdados de `Object` por último —, declarações da
//! biblioteca, importados, prefixos, não importados, palavras-chave) e o
//! nome.

use crate::DocumentStore;
use crate::consulta::Consulta;
use crate::indice::{IndiceProjeto, IndiceSdk};
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

mod alvo;
mod contexto;
mod isp;
mod isp_declaracoes;
mod isp_nao_importados;
mod isp_outros;
mod isp_palavras;
mod isp_visitas;

/// Identificador que ocupa o lugar do nome sob o cursor na análise.
const SENTINELA: &str = "dartforge__completar";

/// Fechos tentados depois do sentinela, na ordem de preferência.
const FECHOS: &[&str] = &["", ";", ")", ");", "))", "));", ")));", "]", "];", "});"];

/// `CompletionItemKind` do LSP.
pub(crate) mod especie {
    pub const METODO: u32 = 2;
    pub const FUNCAO: u32 = 3;
    pub const CONSTRUTOR: u32 = 4;
    pub const CAMPO: u32 = 5;
    pub const VARIAVEL: u32 = 6;
    pub const CLASSE: u32 = 7;
    pub const MODULO: u32 = 9;
    pub const PROPRIEDADE: u32 = 10;
    pub const ENUM: u32 = 13;
    pub const PALAVRA_CHAVE: u32 = 14;
    pub const MEMBRO_DE_ENUM: u32 = 20;
    pub const PARAMETRO_DE_TIPO: u32 = 25;
}

/// Como completar uma chamada, quando o cliente aceita snippets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chamada {
    /// Os parâmetros obrigatórios, na ordem (`x`, e `nome: ` para nomeados):
    /// viram os marcadores `${1:x}` do snippet.
    Parametros(Vec<String>),
    /// Função com parâmetros de nomes desconhecidos (índice de nomes): o
    /// cursor fica entre os parênteses.
    Desconhecida,
}

/// Um `import` a acrescentar junto com o item (importação automática).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportAutomatico {
    /// A URI importada (`dart:math`, `util.dart`).
    pub uri: String,
    /// Onde a diretiva entra no documento (bytes) e o texto dela.
    pub span: Span,
    pub texto: String,
}

/// Um item oferecido ao editor.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemCompletar {
    /// O que a lista mostra (`met(…)`, `nome: `).
    pub rotulo: String,
    /// `CompletionItemKind`.
    pub especie: u32,
    /// Tipo ou assinatura.
    pub detalhe: Option<String>,
    /// Texto que substitui o prefixo digitado.
    pub inserir: String,
    /// Forma de chamada, para funções, métodos e construtores.
    pub chamada: Option<Chamada>,
    /// Arquivo e início da declaração, para a documentação do
    /// `completionItem/resolve`.
    pub origem: Option<(PathBuf, usize)>,
    /// A diretiva que o item acrescenta (nome ainda não importado).
    pub importar: Option<ImportAutomatico>,
    /// Grupo de ordenação (menor vem antes).
    grupo: u8,
    /// Qualidade do casamento com o digitado (0: prefixo; 1: aproximado).
    qualidade: u8,
    /// O que entra no cálculo da relevância (`crate::relevancia`).
    rel: crate::relevancia::Rel,
    /// Classe do item (elemento de topo), para o tipo de contexto.
    classe: Option<ClassId>,
    /// A relevância calculada (`0..1000`).
    relevancia: i32,
    /// `sortText` = `9999 − relevância` (`MAP:59`, `:735-736`).
    pub sort_text: String,
    /// O score do `FuzzyMatcher` do handler (`fuzzy.suggestionScore`), para
    /// o truncamento.
    pub score: f64,
    /// `selectionOffset`/`selectionLength` (unidades UTF-16 do texto
    /// inserido), quando a sugestão põe o cursor antes do fim.
    pub selecao: Option<(usize, usize)>,
    /// `displayText` (o rótulo, quando difere do texto inserido).
    pub exibicao: Option<String>,
    /// O item veio do `InScopeCompletionPass` (a ordem e o `matcherScore`
    /// são os dele).
    pub(crate) do_passe: bool,
    /// `CompletionSuggestionKind.IDENTIFIER` num executável (tear-off,
    /// redirecionamento): o rótulo mostra os parâmetros, a inserção não põe
    /// parênteses.
    pub(crate) identificador: bool,
    /// O texto que o filtro do handler casa (`textToMatchOverride`:
    /// `override_x`, `setState`), quando difere do texto inserido.
    pub(crate) casar: Option<String>,
    /// `replacementLength: 0`: a inserção não substitui o prefixo.
    pub(crate) substituir_vazio: bool,
}

/// Resultado do completar: o intervalo do prefixo (bytes) e os itens em ordem.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completar {
    /// O `replacementOffset` (bytes).
    pub inicio: usize,
    /// O fim do `insert` (`replacementOffset + min(offset -
    /// replacementOffset, replacementLength)`).
    pub fim: usize,
    /// O fim do `replacementRange`, quando o passe o calculou (senão o
    /// handler estende a palavra).
    pub fim_da_substituicao: Option<usize>,
    pub itens: Vec<ItemCompletar>,
    /// A lista foi cortada (nomes não importados acima do teto).
    pub incompleta: bool,
}

/// Onde o sentinela caiu na análise.
#[derive(Clone, Copy)]
enum Sentinela {
    /// Numa expressão; com o alvo quando é `alvo.▮`.
    Expr(ExprId, Option<ExprId>),
    /// Num nome de tipo; com o prefixo quando é `p.▮`, e se o tipo abre a
    /// declaração (de topo, membro ou comando), onde palavras-chave também
    /// cabem.
    Tipo {
        prefixo: Option<SymbolId>,
        lider: Option<&'static [&'static str]>,
    },
}

/// O que o completar consulta além do programa.
pub(crate) struct Indices<'a> {
    pub sdk: &'a IndiceSdk,
    pub projeto: &'a mut IndiceProjeto,
    /// Os resumos das bibliotecas conhecidas (`FileStateFilter` e
    /// `exportNamespace`) dos não importados.
    pub conhecidas: &'a mut crate::conhecidas::IndiceDeBibliotecas,
}

mod grupo {
    pub const NOMEADO: u8 = 0;
    pub const LOCAL: u8 = 1;
    pub const MEMBRO: u8 = 2;
    /// Membros herdados de `Object` (`toString`, `hashCode`…).
    pub const MEMBRO_DE_OBJECT: u8 = 3;
    pub const BIBLIOTECA: u8 = 4;
    pub const IMPORTADO: u8 = 5;
    pub const PREFIXO: u8 = 6;
    pub const NAO_IMPORTADO: u8 = 7;
    pub const PALAVRA: u8 = 8;
}

/// Teto de itens de bibliotecas não importadas numa resposta; acima dele a
/// lista sai com `isIncomplete` e o editor pede de novo a cada tecla.

const PALAVRAS_DE_EXPRESSAO: &[&str] = &["const", "false", "new", "null", "true"];
const PALAVRAS_DE_COMANDO: &[&str] = &[
    "assert", "break", "const", "continue", "do", "false", "final", "for", "if", "late", "new",
    "null", "return", "switch", "throw", "true", "try", "var", "void", "while",
];
const PALAVRAS_DE_TOPO: &[&str] = &[
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
const PALAVRAS_DE_MEMBRO: &[&str] = &[
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

fn eh_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Completa na posição `offset` (bytes) de `texto`, o conteúdo vigente de `uri`.
///
/// `None` quando o arquivo não pode ser carregado (URI que não é de
/// arquivo). Dentro de comentário, de texto de string ou de número, a lista
/// é vazia.
/// O `maxSuggestions` (`dart.maxCompletionItems`, padrão
/// `defaultMaxCompletions = 2000`).
pub(crate) const MAXIMO_PADRAO: usize = 2000;

/// `SuggestionCollector.addSuggestion` (`SC:45-81`): inserção estável por
/// `matcherScore` decrescente e a poda acima de `maximo`.
fn coletar(lista: &mut Vec<(f64, ItemCompletar)>, score: f64, item: ItemCompletar, maximo: usize) {
    let mut posicao = 0;
    for k in (0..lista.len()).rev() {
        if lista[k].0 >= score {
            posicao = k + 1;
            break;
        }
    }
    lista.insert(posicao, (score, item));
    if lista.len() > maximo {
        let minimo = lista[maximo].0;
        while lista.len() > maximo && lista.last().is_some_and(|(s, _)| *s < minimo) {
            lista.pop();
        }
    }
}

/// O texto pontuado (`displayName`): o rótulo de inserção sem o `: ` de um
/// argumento nomeado.
fn texto_pontuado(i: &ItemCompletar) -> &str {
    match &i.casar {
        Some(c) => c,
        None => i.inserir.trim_end_matches([':', ' ']),
    }
}

pub(crate) fn completar(
    sdk: &SdkLayout,
    indices: Indices<'_>,
    documentos: &DocumentStore,
    uri: &str,
    texto: &str,
    offset: usize,
    features: LibraryFeatures,
    maximo: usize,
    nao_importados: bool,
    retido: Option<&mut crate::projeto::Projeto>,
) -> Option<Completar> {
    let bytes = texto.as_bytes();
    let offset = offset.min(texto.len());
    let mut inicio = offset;
    while inicio > 0 && eh_ident(bytes[inicio - 1]) {
        inicio -= 1;
    }
    let mut fim = offset;
    while fim < bytes.len() && eh_ident(bytes[fim]) {
        fim += 1;
    }
    let vazio = Completar {
        inicio: offset,
        fim: offset,
        fim_da_substituicao: None,
        itens: Vec::new(),
        incompleta: false,
    };
    // O texto analisado troca a palavra sob o cursor pelo sentinela (só para
    // a semântica: o contexto é o do texto real, `completar/isp.rs`).
    let digitado = &texto[inicio..offset];
    let preparado = preparar(texto, inicio, fim, features);
    let texto_analisado = preparado.as_deref().unwrap_or(texto);
    let delta = texto_analisado.len() as isize - texto.len() as isize;
    let pedido = Pedido { sdk, indices, documentos, texto, inicio, fim, offset, digitado, especulativo: preparado.is_some(), maximo, nao_importados, delta };
    // O caminho incremental (docs/LSP-ESPECIFICACAO.md §16.7): o estado
    // retido é o do texto do documento; o texto com o sentinela troca a
    // unidade no lugar, só o corpo do sentinela é inferido (com a sonda), e a
    // troca é desfeita depois da resposta. Fora de um corpo, ou com a
    // assinatura, as diretivas ou a forma mudadas, a carga completa.
    if let Some(projeto) = retido
        && let Some(unidade) = projeto.unidade_do_uri(uri)
        && projeto.programa().unit(unidade).source == texto
        && projeto.bibliotecas.contains(&projeto.programa().unit(unidade).library)
    {
        let consulta = &mut projeto.consulta;
        match preparado.as_deref() {
            None => return responder(consulta, unidade, pedido, vazio),
            Some(especulativo) => {
                if let Some(troca) = consulta.trocar_unidade(unidade, especulativo, crate::consulta::ModoDeTroca::Especulativa { offset: inicio }) {
                    let resposta = responder(consulta, unidade, pedido, vazio);
                    consulta.desfazer(troca);
                    return resposta;
                }
            }
        }
    }
    let (programa, nomes, unidade) =
        crate::semantica::carregar(sdk, uri, texto_analisado, Some(documentos))?;
    let biblioteca = programa.unit(unidade).library;
    let sonda = preparado.as_ref().map(|_| (unidade, inicio));
    let mut consulta = Consulta::inferir(programa, nomes, &[biblioteca], false, sonda);
    responder(&mut consulta, unidade, pedido, vazio)
}

/// O pedido de completar, já com o intervalo da palavra.
struct Pedido<'a, 'i> {
    sdk: &'a SdkLayout,
    indices: Indices<'i>,
    documentos: &'a DocumentStore,
    texto: &'a str,
    inicio: usize,
    fim: usize,
    offset: usize,
    digitado: &'a str,
    /// O texto analisado tem o sentinela.
    especulativo: bool,
    maximo: usize,
    /// Sugerir os não importados (`suggestFromUnimportedLibraries` com
    /// `workspace/applyEdit`).
    nao_importados: bool,
    /// O deslocamento dos offsets depois da palavra no texto analisado.
    delta: isize,
}

/// As sugestões sobre a consulta pronta (a retida, com a unidade trocada
/// pelo texto especulativo, ou a de uma carga): o `InScopeCompletionPass`
/// sobre o texto real (`completar/isp.rs`), o `NotImportedCompletionPass`,
/// a relevância de cada candidato, o coletor (`matcherScore`, poda), o
/// mapa de sugestões e o filtro e o truncamento do handler.
fn responder(consulta: &mut Consulta, unidade: dartforge_elements::model::UnitId, pedido: Pedido<'_, '_>, vazio: Completar) -> Option<Completar> {
    let Pedido { sdk, indices, documentos, texto, inicio, fim, offset, digitado: _, especulativo, maximo, nao_importados, delta } = pedido;
    let features = consulta.programa.unit(unidade).features;
    let antes_de_3 = features.versao() < dartforge_frontend::LanguageVersion::new(3, 0);
    let troca = especulativo.then_some((inicio, fim, SENTINELA.len(), delta));
    // Numa parte, o `import` iria para a biblioteca dona (outro arquivo).
    let parte = consulta.programa.unit(unidade).unit.directives.iter().any(|d| matches!(d.kind, ast::DirectiveKind::PartOf { .. }));
    // As bibliotecas candidatas dos não importados (`FileStateFilter`, menos
    // a do pedido e as importadas sem combinadores pela unidade definidora,
    // `_ImportSummary`), com a URI do `import`.
    let filtro = candidatas_filtro(consulta, unidade);
    let Indices { sdk: _, projeto: _, conhecidas } = indices;
    let candidatas = |c: &mut crate::conhecidas::IndiceDeBibliotecas| -> Vec<(PathBuf, String)> {
        let Some((arquivo, proprias, importadas)) = &filtro else { return Vec::new() };
        let raiz = crate::projeto::raiz_do_projeto(arquivo);
        let pacote = crate::indice::nome_do_pacote(&raiz);
        let chave = |x: &std::path::Path| dartforge_elements::gerado::chave(x);
        let mut v = Vec::new();
        for candidata in crate::conhecidas::candidatas(arquivo, Some(sdk), c, documentos) {
            let k = chave(&candidata.caminho);
            if proprias.contains(&k) || importadas.contains(&k) {
                continue;
            }
            let uri = if candidata.sdk || candidata.uri.starts_with("package:") {
                candidata.uri.clone()
            } else {
                match crate::indice::uri_de_import(arquivo, &candidata.caminho, &raiz, pacote.as_deref()) {
                    Some(u) => u,
                    None => continue,
                }
            };
            v.push((candidata.caminho, uri));
        }
        v
    };
    let r = {
        let mut lista = || candidatas(conhecidas);
        let nao_importadas: Option<&mut dyn FnMut() -> Vec<(PathBuf, String)>> = if parte || !nao_importados { None } else { Some(&mut lista) };
        isp::executar(consulta, unidade, texto, features, offset, troca, antes_de_3, nao_importadas)
    };
    if r.comentario {
        return Some(vazio);
    }
    let biblioteca = consulta.programa.unit(unidade).library;
    // O sentinela é do texto analisado, não do usuário.
    let mut chegada: Vec<(f64, ItemCompletar)> = r.itens.into_iter().filter(|(_, i)| !i.inserir.contains(SENTINELA)).collect();
    // `StaticMembersOperation` sobre as bibliotecas que a consulta não
    // carregou: os nomes do resumo do `exportNamespace`.
    if r.operacoes.iter().any(|o| matches!(o, isp::Operacao::MembrosEstaticos)) && !r.resumidas.is_empty() {
        let so_tipos = r.flags.as_ref().is_some_and(|f| f.tipo);
        let mut coletor = Coletor { itens: Vec::new(), biblioteca, receptor: None };
        let mut nomes = Interner::new();
        let real = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
        if let Some((arquivo, _, _)) = &filtro {
            coletor.nao_importados_resumo(texto, &real.unit, so_tipos, sdk, arquivo, conhecidas, documentos, &r.resumidas, &r.visiveis);
        }
        let mut casador = (!r.prefixo_do_casador.is_empty()).then(|| crate::casador::Casador::novo(&r.prefixo_do_casador, crate::casador::Estilo::Texto));
        for i in coletor.itens {
            let s = match casador.as_mut() {
                Some(c) => c.score(i.inserir.as_str()),
                None => 0.0,
            };
            if s != -1.0 {
                chegada.push((s, i));
            }
        }
    }
    // A relevância (`relevanceComputer.computeRelevance`): o local gravado
    // pelo passe, o tipo de contexto do pedido e o `preferConstants`.
    let local = r.local.clone();
    let esperado = r.tipo_de_contexto;
    let preferir_constantes = r.prefere_constantes || r.em_contexto_constante;
    let tipo_bool = consulta.core.bool_;
    let tipo_nulo = consulta.core.null;
    for (_, i) in &mut chegada {
        if i.rel.tipo.is_none() {
            i.rel.tipo = match i.rel.palavra {
                Some("true" | "false") => Some(tipo_bool),
                Some("null") => Some(tipo_nulo),
                _ => None,
            };
        }
        if i.rel.tipo.is_none()
            && let Some(c) = i.classe
        {
            i.rel.tipo = tipo_da_classe(consulta, c);
        }
        let contexto = match (esperado, i.rel.tipo) {
            (Some(e), Some(t)) => caracteristica_de_contexto(consulta, e, t),
            _ => 0.0,
        };
        i.relevancia = crate::relevancia::relevancia(&i.rel, local.as_deref(), contexto, preferir_constantes);
        i.sort_text = (9999 - i.relevancia).to_string();
    }
    // O coletor: inserção estável por `matcherScore` e a poda.
    let mut lista: Vec<(f64, ItemCompletar)> = Vec::new();
    for (s, i) in chegada {
        coletar(&mut lista, s, i, maximo);
    }
    // O `_suggestionMap` (`SB:989-1026`): a chave é o texto (o construtor
    // com `()`, o não importado com `::uri`); o último vence, na posição do
    // primeiro; o construtor cujo nome de classe já está no mapa por um
    // elemento que não é classe fica de fora.
    let mut ordem: Vec<ItemCompletar> = Vec::new();
    let mut indice: HashMap<String, usize> = HashMap::new();
    for (_, i) in lista {
        if i.especie == especie::CONSTRUTOR
            && let Some(c) = i.classe
        {
            let nome_da_classe = consulta.nome(consulta.programa.class(c).name).to_string();
            if let Some(&k) = indice.get(&nome_da_classe) {
                let existente = &ordem[k];
                let e_classe = existente.classe.is_some_and(|x| {
                    let cl = consulta.programa.class(x);
                    matches!(cl.kind, ClassKind::Class | ClassKind::MixinApplication)
                }) && existente.especie == especie::CLASSE;
                if !e_classe {
                    continue;
                }
            }
        }
        let mut chave = i.inserir.clone();
        if i.especie == especie::CONSTRUTOR {
            chave.push_str("()");
        }
        if let Some(imp) = &i.importar
            && i.rel.nao_importado
        {
            chave.push_str("::");
            chave.push_str(&imp.uri);
        }
        match indice.get(&chave) {
            Some(&k) => ordem[k] = i,
            None => {
                indice.insert(chave, ordem.len());
                ordem.push(i);
            }
        }
    }
    // O handler: `fuzzy.suggestionScore(item) > 0` pelo prefixo do pedido
    // (vazio → 1.0).
    let mut fuzzy = crate::casador::Casador::novo(&r.prefixo_do_pedido, crate::casador::Estilo::Texto);
    for i in &mut ordem {
        i.score = fuzzy.score(texto_pontuado(i));
    }
    ordem.retain(|i| i.score > 0.0);
    let mut incompleta = r.incompleta;
    // `_truncateResults` acima do máximo.
    if ordem.len() > maximo {
        let prefixo = r.prefixo_do_pedido.to_lowercase();
        ordem.sort_by(|a, b| {
            if a.score != b.score {
                return b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal);
            }
            if a.sort_text == b.sort_text {
                return a.rotulo.len().cmp(&b.rotulo.len());
            }
            a.sort_text.cmp(&b.sort_text)
        });
        let mut k = 0usize;
        ordem.retain(|i| {
            let manter = k < maximo || (!prefixo.is_empty() && texto_pontuado(i).to_lowercase() == prefixo);
            k += 1;
            manter
        });
        incompleta = true;
    }
    // `replacementRange` e o `insert` do handler (`min(offset -
    // replacementOffset, replacementLength)`).
    let (ri, rl) = r.intervalo;
    let insercao = ri + offset.saturating_sub(ri).min(rl);
    Some(Completar { inicio: ri, fim: insercao, fim_da_substituicao: Some(ri + rl), itens: ordem, incompleta })
}

/// O que o filtro das candidatas dos não importados precisa: o arquivo do
/// pedido, as unidades da própria biblioteca e as bibliotecas importadas
/// sem combinadores pela unidade definidora (`_ImportSummary`).
#[allow(clippy::type_complexity)]
fn candidatas_filtro(consulta: &Consulta, unidade: dartforge_elements::model::UnitId) -> Option<(PathBuf, HashSet<PathBuf>, HashSet<PathBuf>)> {
    let programa = &consulta.programa;
    let lib = programa.library(programa.unit(unidade).library);
    let arquivo = programa.unit(unidade).path.clone()?;
    let definidora = lib.units.first().copied();
    let chave = |c: &std::path::Path| dartforge_elements::gerado::chave(c);
    let importadas: HashSet<PathBuf> = lib
        .imports
        .iter()
        .filter(|i| Some(i.unit) == definidora && i.combinators.is_empty())
        .filter_map(|i| programa.library(i.library).units.first())
        .filter_map(|u| programa.unit(*u).path.as_deref().map(chave))
        .collect();
    let proprias: HashSet<PathBuf> = lib.units.iter().filter_map(|u| programa.unit(*u).path.as_deref().map(chave)).collect();
    Some((arquivo, proprias, importadas))
}

/// Texto com o nome sob o cursor trocado pelo sentinela (e o fecho que o
/// faz analisar); `None` quando nenhuma variante põe o sentinela numa
/// expressão ou num nome de tipo. Vale a variante com menos diagnósticos
/// (a primeira, entre iguais).
fn preparar(texto: &str, inicio: usize, fim: usize, features: LibraryFeatures) -> Option<String> {
    let mut melhor: Option<(usize, String)> = None;
    for fecho in FECHOS {
        let candidato = format!("{}{SENTINELA}{fecho}{}", &texto[..inicio], &texto[fim..]);
        let mut nomes = Interner::new();
        let analisado = dartforge_frontend::parser::parse_com(&candidato, &mut nomes, features);
        if achar_sentinela(&analisado.ast, inicio).is_none() {
            continue;
        }
        // Sem fecho, o texto tem a árvore do real (o parser insere os
        // fechos sintéticos como o fasta): é a escolha sempre que o
        // sentinela cai numa expressão ou num tipo.
        if fecho.is_empty() {
            return Some(candidato);
        }
        let erros = analisado.diagnostics.len();
        if melhor.as_ref().is_none_or(|(e, _)| erros < *e) {
            let zero = erros == 0;
            melhor = Some((erros, candidato));
            if zero {
                break;
            }
        }
    }
    melhor.map(|(_, t)| t)
}

/// Onde está o sentinela em `inicio`: numa expressão (com o alvo, se é
/// acesso a membro) ou num nome de tipo.
fn achar_sentinela(ast: &ast::Ast, inicio: usize) -> Option<Sentinela> {
    let eh = |n: &ast::Name| n.span.start == inicio && n.span.end == inicio + SENTINELA.len();
    let expr = ast
        .exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Identifier(n) if eh(n) => Some(Sentinela::Expr(ExprId(i as u32), None)),
            ExprKind::Property { target, name, .. } if eh(name) => {
                Some(Sentinela::Expr(ExprId(i as u32), Some(*target)))
            }
            _ => None,
        });
    if expr.is_some() {
        return expr;
    }
    let (tipo, nome) = ast.types.iter().find_map(|t| match &t.kind {
        ast::TypeKind::Named { name, .. } if name.last().is_some_and(eh) => Some((t, name)),
        _ => None,
    })?;
    let prefixo = (nome.len() == 2).then(|| nome[0].sym);
    // O tipo abre uma declaração de topo, de membro ou um comando: ali as
    // palavras-chave de declaração também cabem.
    let abre = |s: dartforge_diagnostics::Span| s.start == tipo.span.start;
    let lider: Option<&'static [&'static str]> = if prefixo.is_some() {
        None
    } else if ast.decls.iter().any(|d| abre(d.span)) {
        Some(PALAVRAS_DE_TOPO)
    } else if ast.members.iter().any(|m| abre(m.span)) {
        Some(PALAVRAS_DE_MEMBRO)
    } else if ast.stmts.iter().any(|s| abre(s.span)) {
        Some(PALAVRAS_DE_COMANDO)
    } else {
        None
    };
    Some(Sentinela::Tipo { prefixo, lider })
}

struct Coletor {
    itens: Vec<ItemCompletar>,
    biblioteca: LibraryId,
    /// Classe do receptor enquanto se coletam membros de instância (a
    /// distância de herança de cada membro).
    receptor: Option<ClassId>,
}

impl Coletor {
    fn empurrar(
        &mut self,
        grupo: u8,
        especie: u32,
        rotulo: String,
        inserir: String,
        detalhe: Option<String>,
    ) -> &mut ItemCompletar {
        self.itens.push(ItemCompletar {
            rotulo,
            especie,
            detalhe,
            inserir,
            chamada: None,
            origem: None,
            importar: None,
            grupo,
            qualidade: 0,
            rel: crate::relevancia::Rel::default(),
            classe: None,
            relevancia: 0,
            sort_text: String::new(),
            score: 0.0,
            selecao: None,
            exibicao: None,
            do_passe: false,
            identificador: false,
            casar: None,
            substituir_vazio: false,
        });
        self.itens.last_mut().expect("item recém-empurrado")
    }

    /// Preenche a relevância do último item a partir da função `f` (com o
    /// tipo `tipo` visto pelo receptor).
    fn rel_de_funcao(&mut self, consulta: &Consulta, f: FunctionElementId, tipo: TypeId) {
        use crate::relevancia::Especie;
        let fe = consulta.programa.function(f);
        let membro = fe.class.is_some() || fe.extension.is_some();
        let nome = consulta.nome(fe.name).to_string();
        let constante_de_enum = fe.variable.is_some_and(|v| {
            let ve = consulta.programa.variable(v);
            ve.class.is_some_and(|c| consulta.programa.class(c).enum_constants.contains(&v))
        });
        let (especie, tipo_do_item) = match fe.kind {
            _ if constante_de_enum => (Especie::SemTabela, Some(tipo)),
            FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                (if membro { Especie::Campo } else { Especie::VariavelDeTopo }, Some(tipo))
            }
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => (Especie::Construtor, retorno(consulta, tipo)),
            FunctionKind::Function | FunctionKind::Operator => {
                (if membro { Especie::Metodo } else { Especie::Funcao }, retorno(consulta, tipo))
            }
        };
        let distancia = match (self.receptor, fe.class) {
            (Some(r), Some(c)) if membro => distancia_de_heranca(consulta, r, c).map(|d| d as u32),
            _ => None,
        };
        // `isConstantFeature`: construtor `const`; acessor sintético de
        // variável estática (ou de topo) `const`.
        let constante = match fe.kind {
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => fe.const_,
            FunctionKind::ImplicitAccessor => fe.variable.is_some_and(|v| {
                let ve = consulta.programa.variable(v);
                ve.const_ && (ve.static_ || (ve.class.is_none() && ve.extension.is_none()))
            }),
            _ => false,
        };
        let obsoleto = obsoleto_da_funcao(consulta, f);
        if let Some(item) = self.itens.last_mut() {
            item.rel.especie = Some(especie);
            item.rel.tipo = tipo_do_item;
            item.rel.distancia = distancia;
            item.rel.comeca_com_dolar = nome.starts_with('$');
            item.rel.no_such_method = nome == "noSuchMethod";
            item.rel.constante = constante;
            item.rel.obsoleto = obsoleto;
        }
    }

    /// Nome privado de outra biblioteca: invisível aqui.
    fn invisivel(&self, consulta: &Consulta, nome: &str, dona: LibraryId) -> bool {
        let _ = consulta;
        nome.starts_with('_') && dona != self.biblioteca
    }

    /// Item de uma função (método, getter, acessor) com o tipo `tipo` já
    /// visto pelo receptor.
    fn funcao(&mut self, consulta: &Consulta, grupo: u8, f: FunctionElementId, tipo: TypeId) {
        let fe = consulta.programa.function(f);
        let nome = consulta.nome(fe.name).trim_end_matches('=').to_string();
        if nome.is_empty()
            || !nome.as_bytes()[0].is_ascii_alphabetic() && !nome.starts_with(['_', '$'])
        {
            return;
        }
        if self.invisivel(consulta, &nome, fe.library) {
            return;
        }
        match fe.kind {
            FunctionKind::ImplicitAccessor => {
                let constante_de_enum = fe.variable.is_some_and(|v| {
                    let ve = consulta.programa.variable(v);
                    ve.class
                        .is_some_and(|c| consulta.programa.class(c).enum_constants.contains(&v))
                });
                let especie = if constante_de_enum {
                    especie::MEMBRO_DE_ENUM
                } else if fe.class.is_some() || fe.extension.is_some() {
                    especie::CAMPO
                } else {
                    especie::VARIAVEL
                };
                let origem = consulta.origem(consulta.inicio_da_funcao(f));
                self.empurrar(
                    grupo,
                    especie,
                    nome.clone(),
                    nome,
                    Some(consulta.formatar(tipo)),
                )
                .origem = origem;
                self.rel_de_funcao(consulta, f, tipo);
            }
            FunctionKind::Getter | FunctionKind::Setter => {
                let especie = if fe.class.is_some() || fe.extension.is_some() {
                    especie::PROPRIEDADE
                } else {
                    especie::VARIAVEL
                };
                let origem = consulta.origem(consulta.inicio_da_funcao(f));
                self.empurrar(
                    grupo,
                    especie,
                    nome.clone(),
                    nome,
                    Some(consulta.formatar(tipo)),
                )
                .origem = origem;
                self.rel_de_funcao(consulta, f, tipo);
            }
            FunctionKind::Function
            | FunctionKind::Constructor
            | FunctionKind::SyntheticConstructor => {
                let especie = match fe.kind {
                    FunctionKind::Function if fe.class.is_some() || fe.extension.is_some() => {
                        especie::METODO
                    }
                    FunctionKind::Function => especie::FUNCAO,
                    _ => especie::CONSTRUTOR,
                };
                let sem_parametros = matches!(consulta.tabela.get(tipo), Type::Function { positional, optional, named, .. }
                    if positional.is_empty() && optional.is_empty() && named.is_empty());
                let rotulo = if sem_parametros {
                    format!("{nome}()")
                } else {
                    format!("{nome}(…)")
                };
                // Os obrigatórios viram marcadores do snippet: posicionais
                // pelo nome e nomeados como `nome: `.
                let parametros: Vec<String> = consulta.outline.functions[f.0 as usize]
                    .parameters
                    .iter()
                    .filter_map(|p| {
                        let n = consulta.nome(p.externo.or(p.name)?).to_string();
                        match p.kind {
                            ast::ParameterKind::Required => Some(n),
                            ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                            _ => None,
                        }
                    })
                    .collect();
                let origem = consulta.origem(consulta.inicio_da_funcao(f));
                let item = self.empurrar(
                    grupo,
                    especie,
                    rotulo,
                    nome,
                    Some(consulta.detalhe_de_funcao(f, tipo)),
                );
                item.chamada = Some(Chamada::Parametros(parametros));
                item.origem = origem;
                self.rel_de_funcao(consulta, f, tipo);
            }
            FunctionKind::Operator => {}
        }
    }

    /// Item de um elemento de topo (da biblioteca ou importado).
    fn elemento(&mut self, consulta: &Consulta, grupo: u8, elemento: Element) {
        let programa = &consulta.programa;
        match elemento {
            Element::Class(c) => {
                let classe = programa.class(c);
                let nome = consulta.nome(classe.name).to_string();
                if self.invisivel(consulta, &nome, classe.library)
                    || classe.kind == ClassKind::MixinApplication && classe.decl.is_none()
                {
                    return;
                }
                let especie = if classe.kind == ClassKind::Enum {
                    especie::ENUM
                } else {
                    especie::CLASSE
                };
                let especie_rel = match classe.decl.map(|d| &programa.unit(d.unit).ast.decl(d.decl).kind) {
                    Some(ast::DeclKind::Enum(_)) => crate::relevancia::Especie::Enum,
                    Some(ast::DeclKind::Mixin(_)) => crate::relevancia::Especie::Mixin,
                    Some(ast::DeclKind::ExtensionType(_)) => crate::relevancia::Especie::SemTabela,
                    _ => crate::relevancia::Especie::Classe,
                };
                let origem = consulta.origem(consulta.inicio_do_elemento(elemento));
                let obsoleto = obsoleto_do_elemento(consulta, elemento);
                let item = self.empurrar(grupo, especie, nome.clone(), nome, None);
                item.origem = origem;
                item.rel.especie = Some(especie_rel);
                item.rel.obsoleto = obsoleto;
                item.classe = Some(c);
            }
            Element::Typedef(t) => {
                let nome = consulta.nome(programa.typedef(t).name).to_string();
                if !self.invisivel(consulta, &nome, programa.typedef(t).library) {
                    let origem = consulta.origem(consulta.inicio_do_elemento(elemento));
                    let alvo_funcao = matches!(consulta.tabela.get(consulta.outline.typedefs[t.0 as usize].target_type), Type::Function { .. });
                    let item = self.empurrar(grupo, especie::CLASSE, nome.clone(), nome, None);
                    item.origem = origem;
                    item.rel.especie = Some(if alvo_funcao {
                        crate::relevancia::Especie::AliasDeFuncao
                    } else {
                        crate::relevancia::Especie::SemTabela
                    });
                }
            }
            Element::Extension(x) => {
                let Some(n) = programa.extension(x).name else {
                    return;
                };
                let nome = consulta.nome(n).to_string();
                if !self.invisivel(consulta, &nome, programa.extension(x).library) {
                    let origem = consulta.origem(consulta.inicio_do_elemento(elemento));
                    let item = self.empurrar(grupo, especie::CLASSE, nome.clone(), nome, None);
                    item.origem = origem;
                    item.rel.especie = Some(crate::relevancia::Especie::SemTabela);
                }
            }
            Element::Function(f) => {
                let dados = &consulta.outline.functions[f.0 as usize];
                let tipo = match programa.function(f).kind {
                    FunctionKind::Getter | FunctionKind::ImplicitAccessor => dados.return_type,
                    FunctionKind::Setter => dados
                        .parameters
                        .first()
                        .map_or(consulta.core.dynamic_, |p| p.ty),
                    _ => dados.signature,
                };
                let tipo = match programa
                    .function(f)
                    .variable
                    .and_then(|v| consulta.tipo_da_variavel(v))
                {
                    Some(t) if programa.function(f).kind == FunctionKind::ImplicitAccessor => t,
                    _ => tipo,
                };
                self.funcao(consulta, grupo, f, tipo);
            }
            Element::Variable(v) => {
                let ve = programa.variable(v);
                let nome = consulta.nome(ve.name).to_string();
                if !self.invisivel(consulta, &nome, ve.library) {
                    let detalhe = consulta.tipo_da_variavel(v).map(|t| consulta.formatar(t));
                    let origem = consulta.origem(consulta.inicio_da_variavel(v));
                    let obsoleto = obsoleto_do_elemento(consulta, elemento);
                    let item = self.empurrar(grupo, especie::VARIAVEL, nome.clone(), nome, detalhe);
                    item.origem = origem;
                    item.rel.especie = Some(crate::relevancia::Especie::VariavelDeTopo);
                    item.rel.tipo = consulta.tipo_da_variavel(v);
                    item.rel.constante = ve.const_;
                    item.rel.obsoleto = obsoleto;
                }
            }
            Element::Prefix(_, p) => {
                let nome = consulta.nome(p).to_string();
                self.empurrar(grupo::PREFIXO, especie::MODULO, nome.clone(), nome, None)
                    .rel
                    .especie = Some(crate::relevancia::Especie::Prefixo);
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
    fn nao_importados_resumo(
        &mut self,
        texto: &str,
        unit: &ast::CompilationUnit,
        so_tipos: bool,
        sdk: &SdkLayout,
        arquivo: &std::path::Path,
        conhecidas: &mut crate::conhecidas::IndiceDeBibliotecas,
        documentos: &DocumentStore,
        candidatas: &[(PathBuf, String)],
        visiveis: &HashSet<String>,
    ) {
        use crate::conhecidas::Especie as E;
        let resolvedor = crate::conhecidas::Resolvedor {
            sdk: Some(sdk),
            pacotes: dartforge_elements::config::PackageConfig::discover(arquivo).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()),
        };
        for (caminho, uri) in candidatas {
            for (nome, origem, d) in conhecidas.exportados(caminho, &resolvedor, documentos) {
                let tipo = crate::conhecidas::DE_TIPO.contains(&d.especie);
                if (so_tipos && !tipo) || visiveis.contains(&nome) {
                    continue;
                }
                let (especie_lsp, rel) = match d.especie {
                    E::Classe | E::TipoDeExtensao => (especie::CLASSE, crate::relevancia::Especie::Classe),
                    E::Mixin => (especie::CLASSE, crate::relevancia::Especie::Mixin),
                    E::Enum => (especie::ENUM, crate::relevancia::Especie::Enum),
                    E::AliasDeFuncao => (especie::CLASSE, crate::relevancia::Especie::AliasDeFuncao),
                    E::AliasDeTipo | E::Extensao => (especie::CLASSE, crate::relevancia::Especie::SemTabela),
                    E::Funcao => (especie::FUNCAO, crate::relevancia::Especie::Funcao),
                    E::Variavel => (especie::VARIAVEL, crate::relevancia::Especie::VariavelDeTopo),
                };
                let funcao = d.especie == E::Funcao;
                let rotulo = match (funcao, d.sem_parametros) {
                    (true, true) => format!("{nome}()"),
                    (true, false) => format!("{nome}(…)"),
                    _ => nome.clone(),
                };
                let (span, novo) = crate::acoes::inserir_import(texto, unit, uri);
                // A assinatura escrita do resumo é o `detail` (o `Auto
                // import from` vem no `completionItem/resolve`).
                let item = self.empurrar(grupo::NAO_IMPORTADO, especie_lsp, rotulo, nome.clone(), d.assinatura.as_deref().map(str::to_string));
                item.importar = Some(ImportAutomatico { uri: uri.clone(), span, texto: novo });
                item.origem = Some((origem, d.inicio));
                item.rel.nao_importado = true;
                item.rel.especie = Some(rel);
                item.do_passe = true;
                if funcao {
                    item.chamada = Some(if d.sem_parametros { Chamada::Parametros(Vec::new()) } else { Chamada::Desconhecida });
                }
            }
        }
    }

}

/// Os nomes das anotações (o último segmento) de uma lista de metadados.
fn nomes_de_anotacoes(consulta: &Consulta, ms: &[ast::Annotation]) -> Vec<String> {
    ms.iter().filter_map(|m| m.name.last().map(|n| consulta.nome(n.sym).to_string())).collect()
}

/// `@deprecated`/`@Deprecated(...)` entre os metadados.
fn tem_deprecated(consulta: &Consulta, ms: &[ast::Annotation]) -> bool {
    nomes_de_anotacoes(consulta, ms).iter().any(|n| n == "deprecated" || n == "Deprecated")
}

/// `hasDeprecated` de um elemento de topo.
fn obsoleto_do_elemento(consulta: &Consulta, el: Element) -> bool {
    let p = &consulta.programa;
    let decl = match el {
        Element::Class(c) => p.class(c).decl,
        Element::Extension(x) => Some(p.extension(x).decl),
        Element::Typedef(t) => Some(p.typedef(t).decl),
        Element::Function(f) => return obsoleto_da_funcao(consulta, f),
        Element::Variable(v) => match p.variable(v).node {
            dartforge_elements::model::VariableRef::TopLevel { unit, decl, .. } => Some(dartforge_elements::model::DeclRef { unit, decl }),
            _ => None,
        },
        Element::Prefix(..) => None,
    };
    decl.is_some_and(|d| tem_deprecated(consulta, &p.unit(d.unit).ast.decl(d.decl).metadata))
}

/// `hasOrInheritsDeprecated` de uma função, método, acessor ou construtor:
/// a anotação dela, ou a do membro de mesmo nome que ela sobrescreve.
fn obsoleto_da_funcao(consulta: &Consulta, f: FunctionElementId) -> bool {
    let p = &consulta.programa;
    let proprio = |f: FunctionElementId| -> bool {
        match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let a = &p.unit(unit).ast;
                a.members
                    .iter()
                    .find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x == function))
                    .map(|m| tem_deprecated(consulta, &m.metadata))
                    .or_else(|| a.decls.iter().find(|d| matches!(d.kind, ast::DeclKind::Function(x) if x == function)).map(|d| tem_deprecated(consulta, &d.metadata)))
                    .unwrap_or(false)
            }
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => tem_deprecated(consulta, &p.unit(unit).ast.member(member).metadata),
            dartforge_elements::model::FunctionRef::None => match p.function(f).variable.map(|v| p.variable(v).node) {
                Some(dartforge_elements::model::VariableRef::Field { unit, member, .. }) => tem_deprecated(consulta, &p.unit(unit).ast.member(member).metadata),
                Some(dartforge_elements::model::VariableRef::TopLevel { unit, decl, .. }) => tem_deprecated(consulta, &p.unit(unit).ast.decl(decl).metadata),
                Some(dartforge_elements::model::VariableRef::EnumConstant { unit, decl, index }) => match &p.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Enum(e) => e.constants.get(index).is_some_and(|k| tem_deprecated(consulta, &k.metadata)),
                    _ => false,
                },
                _ => false,
            },
        }
    };
    if proprio(f) {
        return true;
    }
    // O membro sobrescrito (`inheritsDeprecated`): o de mesmo nome nos
    // supertipos.
    let fe = p.function(f);
    let Some(c) = fe.class else { return false };
    if fe.static_ || matches!(fe.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
        return false;
    }
    let mut pilha: Vec<ClassId> = Vec::new();
    let cl = p.class(c);
    pilha.extend(cl.supertype_class);
    pilha.extend(cl.mixin_classes.iter().copied());
    pilha.extend(cl.interface_classes.iter().copied());
    pilha.extend(cl.on_classes.iter().copied());
    let mut vistas = HashSet::new();
    while let Some(x) = pilha.pop() {
        if !vistas.insert(x) {
            continue;
        }
        let s = p.class(x);
        if let Some(&g) = s.instance_members.get(&fe.name)
            && proprio(g)
        {
            return true;
        }
        pilha.extend(s.supertype_class);
        pilha.extend(s.mixin_classes.iter().copied());
        pilha.extend(s.interface_classes.iter().copied());
        pilha.extend(s.on_classes.iter().copied());
    }
    false
}

/// Tipo declarado de um membro, sem receptor (estáticos e extensões).
fn tipo_declarado(consulta: &Consulta, f: FunctionElementId) -> TypeId {
    let fe = consulta.programa.function(f);
    let dados = &consulta.outline.functions[f.0 as usize];
    match fe.kind {
        FunctionKind::ImplicitAccessor => fe
            .variable
            .and_then(|v| consulta.tipo_da_variavel(v))
            .unwrap_or(dados.return_type),
        FunctionKind::Getter => dados.return_type,
        FunctionKind::Setter => dados
            .parameters
            .first()
            .map_or(consulta.core.dynamic_, |p| p.ty),
        _ => dados.signature,
    }
}

/// Biblioteca que declara um elemento de topo.
fn biblioteca_do_elemento(consulta: &Consulta, elemento: Element) -> LibraryId {
    let p = &consulta.programa;
    match elemento {
        Element::Class(c) => p.class(c).library,
        Element::Extension(x) => p.extension(x).library,
        Element::Typedef(t) => p.typedef(t).library,
        Element::Function(f) => p.function(f).library,
        Element::Variable(v) => p.variable(v).library,
        Element::Prefix(l, _) => l,
    }
}

/// O retorno de um tipo de função.
fn retorno(consulta: &Consulta, tipo: TypeId) -> Option<TypeId> {
    match consulta.tabela.get(tipo) {
        Type::Function { ret, .. } => Some(*ret),
        _ => None,
    }
}

/// Arestas de `sub` até `sup` na hierarquia (superclasse, mixins,
/// interfaces, `on`), como o `_inheritanceDistance` do Dart.
fn distancia_de_heranca(consulta: &Consulta, sub: ClassId, sup: ClassId) -> Option<usize> {
    let mut fila = std::collections::VecDeque::from([(sub, 0usize)]);
    let mut vistas = HashSet::new();
    while let Some((c, d)) = fila.pop_front() {
        if c == sup {
            return Some(d);
        }
        if !vistas.insert(c) {
            continue;
        }
        let cl = consulta.programa.class(c);
        for s in cl.supertype_class.iter().chain(&cl.mixin_classes).chain(&cl.interface_classes).chain(&cl.on_classes) {
            fila.push_back((*s, d + 1));
        }
    }
    None
}

/// O tipo de interface de `c` com os argumentos `Never`, como o Dart
/// instancia uma classe sugerida (`instantiateInstanceElement`).
fn tipo_da_classe(consulta: &mut Consulta, c: ClassId) -> Option<TypeId> {
    let n = consulta.outline.classes.get(c.0 as usize)?.type_params.len();
    let never = consulta.core.never;
    let args: Box<[TypeId]> = std::iter::repeat_n(never, n).collect();
    Some(consulta.tabela.intern(Type::Interface { class: c, args, nullable: false }))
}

/// `contextTypeFeature`: igual 1,0; subtipo 0,40; supertipo 0,02; sem
/// relação 0,13.
fn caracteristica_de_contexto(consulta: &mut Consulta, esperado: TypeId, tipo: TypeId) -> f64 {
    if esperado == tipo {
        return 1.0;
    }
    let Consulta { tabela, outline, core, .. } = consulta;
    let mut env = dartforge_types::SubtypeEnv::new(tabela, &outline.hierarchy, core);
    if dartforge_types::is_subtype(tipo, esperado, &mut env) {
        0.40
    } else if dartforge_types::is_subtype(esperado, tipo, &mut env) {
        0.02
    } else {
        0.13
    }
}

