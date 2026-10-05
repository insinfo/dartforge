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
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, LibraryId, Namespace,
};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::LibraryFeatures;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StmtKind};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

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
}

/// Resultado do completar: o intervalo do prefixo (bytes) e os itens em ordem.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completar {
    pub inicio: usize,
    pub fim: usize,
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

/// A ordem em que o `InScopeCompletionPass` cria os candidatos de cada
/// grupo (a ordem de chegada ao coletor): argumentos nomeados, palavras de
/// expressão, escopo léxico de dentro para fora, membros, topo da
/// biblioteca, prefixos, importados, palavras de comando, não importados.
fn ordem_de_visita(i: &ItemCompletar) -> u8 {
    match i.grupo {
        grupo::NOMEADO => 0,
        grupo::PALAVRA if !PALAVRAS_DE_COMANDO.contains(&i.inserir.as_str()) || PALAVRAS_DE_EXPRESSAO.contains(&i.inserir.as_str()) => 1,
        grupo::LOCAL => 2,
        grupo::MEMBRO => 3,
        grupo::MEMBRO_DE_OBJECT => 4,
        grupo::BIBLIOTECA => 5,
        grupo::PREFIXO => 6,
        grupo::IMPORTADO => 7,
        grupo::PALAVRA => 8,
        _ => 9,
    }
}

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
    i.inserir.trim_end_matches([':', ' '])
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
        inicio,
        fim: offset,
        itens: Vec::new(),
        incompleta: false,
    };
    if bytes.get(inicio).is_some_and(u8::is_ascii_digit) && inicio < offset
        || fora_de_codigo(texto, offset)
    {
        return Some(vazio);
    }
    let digitado = &texto[inicio..offset];
    let preparado = preparar(texto, inicio, fim, features);
    let texto_analisado = preparado.as_deref().unwrap_or(texto);
    let (programa, nomes, unidade) =
        crate::semantica::carregar(sdk, uri, texto_analisado, Some(documentos))?;
    let biblioteca = programa.unit(unidade).library;
    let sonda = preparado.as_ref().map(|_| (unidade, inicio));
    let mut consulta = Consulta::inferir(programa, nomes, &[biblioteca], false, sonda);
    let mut coletor = Coletor {
        itens: Vec::new(),
        biblioteca,
        receptor: None,
    };

    let sentinela = preparado
        .as_ref()
        .and_then(|_| achar_sentinela(&consulta.programa.unit(unidade).ast, inicio));
    // Nome de `show`/`hide`: o que a biblioteca da diretiva exporta.
    let combinador = biblioteca_do_combinador(&consulta, unidade, inicio);
    // Nomes de bibliotecas não importadas cabem onde um nome solto cabe.
    let mut nao_importados: Option<bool> = None;
    // `this.▮` e `super.▮` num construtor: campos ou parâmetros do
    // construtor da superclasse ainda não usados.
    let formal = parametro_formal(&consulta, unidade, inicio);
    match sentinela {
        _ if formal.is_some() => {
            for (nome, tipo) in formal.into_iter().flatten() {
                coletor.empurrar(grupo::MEMBRO, especie::CAMPO, nome.clone(), nome, tipo);
            }
        }
        _ if combinador.is_some() => {
            if let Some(alvo) = combinador {
                let exportado = consulta.programa.library(alvo).exported.clone();
                coletor.espaco_de_nomes(&consulta, &exportado, |_| grupo::IMPORTADO);
            }
        }
        Some(Sentinela::Expr(expr, Some(alvo))) => {
            coletor.membros_do_alvo(&mut consulta, unidade, expr, alvo)
        }
        Some(Sentinela::Expr(expr, None)) => {
            coletor.argumentos_nomeados(&consulta, unidade, expr);
            coletor.escopo(&mut consulta, unidade);
            let ast = &consulta.programa.unit(unidade).ast;
            let comando = ast
                .stmts
                .iter()
                .any(|s| matches!(s.kind, StmtKind::Expression(e) if e == expr));
            let estatico = consulta.escopo.as_ref().is_none_or(|e| e.estatico);
            let mut palavras: Vec<&str> = if comando {
                PALAVRAS_DE_COMANDO.to_vec()
            } else {
                PALAVRAS_DE_EXPRESSAO.to_vec()
            };
            if !estatico {
                palavras.extend(["this", "super"]);
            }
            coletor.palavras(&palavras);
            nao_importados = Some(false);
        }
        Some(Sentinela::Tipo { prefixo, lider }) => {
            coletor.tipos(&consulta, unidade, inicio, prefixo);
            if let Some(palavras) = lider {
                coletor.palavras(palavras);
            }
            if prefixo.is_none() {
                nao_importados = Some(true);
            }
        }
        None if nome_declarado(&consulta.programa.unit(unidade).ast, inicio) => {
            // O nome de uma declaração (variável com `var`/`final`/tipo,
            // parâmetro, função, classe, membro): o servidor do Dart não
            // sugere nada ali.
            return Some(vazio);
        }
        None => {
            coletor.biblioteca(&consulta);
            let ast = &consulta.programa.unit(unidade).ast;
            let em_tipo = ast.decls.iter().any(|d| {
                d.span.start < inicio
                    && inicio < d.span.end
                    && matches!(
                        d.kind,
                        ast::DeclKind::Class(_)
                            | ast::DeclKind::Mixin(_)
                            | ast::DeclKind::Enum(_)
                            | ast::DeclKind::Extension(_)
                            | ast::DeclKind::ExtensionType(_)
                    )
            });
            let em_corpo = ast.functions.iter().any(|f| {
                let corpo = match f.body {
                    ast::FunctionBody::Block(s) => ast.stmt(s).span,
                    ast::FunctionBody::Expression(e) => ast.expr(e).span,
                    _ => return false,
                };
                corpo.start < inicio && inicio <= corpo.end
            });
            let palavras = if em_corpo {
                PALAVRAS_DE_COMANDO
            } else if em_tipo {
                PALAVRAS_DE_MEMBRO
            } else {
                PALAVRAS_DE_TOPO
            };
            coletor.palavras(palavras);
        }
    }
    let mut incompleta = false;
    // Numa parte, o `import` iria para a biblioteca dona (outro arquivo).
    let parte = consulta
        .programa
        .unit(unidade)
        .unit
        .directives
        .iter()
        .any(|d| matches!(d.kind, ast::DirectiveKind::PartOf { .. }));
    if let Some(so_tipos) = nao_importados
        && !digitado.is_empty()
        && !parte
    {
        incompleta = coletor.nao_importados(
            &consulta, unidade, texto, so_tipos, sdk, indices, documentos,
        );
    }

    // A declaração que é a própria palavra sob o cursor (o `cl` solto no
    // topo, que a recuperação do parser completa como `cl;`, variável de
    // topo, como o fasta) não é sugestão: começa no cursor e tem o nome
    // digitado, como o servidor de análise do Dart a exclui.
    let caminho = consulta.programa.unit(unidade).path.clone();
    let palavra = &texto[inicio..fim];
    let mut itens = coletor.itens;
    itens.retain_mut(|i| {
        if i.inserir == SENTINELA {
            return false;
        }
        if i.inserir == palavra
            && i.importar.is_none()
            && caminho.is_some()
            && i.origem.as_ref().is_some_and(|(p, o)| Some(p) == caminho.as_ref() && *o == inicio)
        {
            return false;
        }
        i.qualidade = 0;
        true
    });
    // Relevância como a do servidor do Dart (`crate::relevancia`): o local
    // do completar e o tipo que ele espera.
    {
        let ast = &consulta.programa.unit(unidade).ast;
        let corpos = &consulta.corpos.units[unidade.0 as usize];
        let construtora = |c: ExprId| matches!(corpos.get_resolved(c), Some(Resolved::Constructor(_)));
        let local: Option<String> = match sentinela {
            _ if combinador.is_some() => Some("ShowCombinator_shownName".into()),
            Some(Sentinela::Expr(expr, Some(_))) => {
                let chamada = ast.exprs.iter().any(|e| matches!(&e.kind, ExprKind::Call { target, .. } if *target == expr));
                (!chamada).then(|| "PropertyAccess_propertyName".to_string())
            }
            Some(Sentinela::Expr(expr, None)) => crate::relevancia::local_da_expressao(ast, expr, &construtora),
            Some(Sentinela::Tipo { .. }) => local_do_tipo(ast, inicio),
            None => {
                let em_classe = ast.decls.iter().any(|d| d.span.start < inicio && inicio < d.span.end && matches!(d.kind, ast::DeclKind::Class(_) | ast::DeclKind::Mixin(_) | ast::DeclKind::Enum(_) | ast::DeclKind::Extension(_) | ast::DeclKind::ExtensionType(_)));
                Some(if em_classe { "ClassDeclaration_member" } else { "CompilationUnit_declaration" }.to_string())
            }
        };
        let esperado = match sentinela {
            Some(Sentinela::Expr(expr, alvo)) => {
                // `a.▮` pede o tipo do acesso inteiro; o resto, o da expressão.
                let _ = alvo;
                tipo_esperado(&mut consulta, unidade, expr)
            }
            _ => None,
        };
        let tipo_bool = consulta.core.bool_;
        let tipo_nulo = consulta.core.null;
        for i in &mut itens {
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
                i.rel.tipo = tipo_da_classe(&mut consulta, c);
            }
            let contexto = match (esperado, i.rel.tipo) {
                (Some(e), Some(t)) => caracteristica_de_contexto(&mut consulta, e, t),
                _ => 0.0,
            };
            i.relevancia = crate::relevancia::relevancia(&i.rel, local.as_deref(), contexto);
        }
    }
    // A ordem de chegada (a visita do passe); dentro de um grupo, a ordem
    // em que o coletor local os criou.
    itens.sort_by_key(ordem_de_visita);
    for i in &mut itens {
        i.sort_text = (9999 - i.relevancia).to_string();
    }
    // O coletor: `matcherScore` (prefixo vazio → 0; −1 não entra).
    let mut casador = (!digitado.is_empty()).then(|| crate::casador::Casador::novo(digitado, crate::casador::Estilo::Texto));
    let mut lista: Vec<(f64, ItemCompletar)> = Vec::new();
    for i in itens {
        let s = match casador.as_mut() {
            Some(c) => c.score(texto_pontuado(&i)),
            None => 0.0,
        };
        if s == -1.0 {
            continue;
        }
        coletar(&mut lista, s, i, maximo);
    }
    // O `_suggestionMap` (`SB:989-1026`): a chave é o texto (o construtor
    // com `()`, o não importado com `::uri`); o último vence, na posição do
    // primeiro.
    let mut ordem: Vec<ItemCompletar> = Vec::new();
    let mut indice: HashMap<String, usize> = HashMap::new();
    for (_, i) in lista {
        let mut chave = i.inserir.clone();
        if i.especie == especie::CONSTRUTOR {
            chave.push_str("()");
        }
        if let Some(imp) = &i.importar {
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
    // O handler: `fuzzy.suggestionScore(item) > 0` (prefixo vazio → 1.0).
    let mut fuzzy = crate::casador::Casador::novo(digitado, crate::casador::Estilo::Texto);
    for i in &mut ordem {
        i.score = fuzzy.score(texto_pontuado(i));
    }
    ordem.retain(|i| i.score > 0.0);
    // `_truncateResults` acima do máximo.
    if ordem.len() > maximo {
        let prefixo = digitado.to_lowercase();
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
    let itens = ordem;
    Some(Completar {
        inicio,
        fim: offset,
        itens,
        incompleta,
    })
}

/// Os nomes que cabem num parâmetro `this.▮` (campos de instância da
/// classe ainda não inicializados por outro parâmetro ou inicializador) ou
/// `super.▮` (parâmetros do construtor da superclasse chamado ainda não
/// repassados), com o tipo.
fn parametro_formal(consulta: &Consulta, unidade: dartforge_elements::model::UnitId, inicio: usize) -> Option<Vec<(String, Option<String>)>> {
    let p = &consulta.programa;
    let u = p.unit(unidade);
    let ast = &u.ast;
    let (membro, k, prm) = ast.members.iter().find_map(|m| match &m.kind {
        ast::MemberKind::Constructor(k) => k
            .parameters
            .iter()
            .find(|x| (x.this_ || x.super_) && x.name.is_some_and(|n| n.span.start == inicio))
            .map(|x| (m, k, x)),
        _ => None,
    })?;
    let classe = (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|c| {
        p.class(*c).decl.is_some_and(|d| {
            d.unit == unidade && {
                let s = u.ast.decl(d.decl).span;
                s.start <= membro.span.start && membro.span.end <= s.end
            }
        })
    })?;
    let usados: HashSet<String> = k
        .parameters
        .iter()
        .filter(|x| !std::ptr::eq(*x, prm))
        .filter_map(|x| x.name.map(|n| consulta.nome(n.sym).to_string()))
        .chain(k.initializers.iter().filter_map(|i| match i {
            ast::Initializer::Field { name, .. } => Some(consulta.nome(name.sym).to_string()),
            _ => None,
        }))
        .collect();
    let mut saida = Vec::new();
    if prm.this_ {
        for v in &p.class(classe).fields {
            let ve = p.variable(*v);
            let nome = consulta.nome(ve.name).to_string();
            if ve.static_ || usados.contains(&nome) {
                continue;
            }
            saida.push((nome, consulta.tipo_da_variavel(*v).map(|t| consulta.formatar(t))));
        }
    } else {
        // O construtor chamado: `super.nome(…)` nos inicializadores, senão
        // o sem nome da superclasse.
        let sup = p.class(classe).supertype_class?;
        let nome_ctor = k.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        });
        let alvo = p.class(sup).construtores().into_iter().find(|(n, _)| match nome_ctor {
            Some(Some(s)) => *n == s,
            _ => consulta.nome(*n).is_empty(),
        });
        let (_, f) = alvo?;
        for q in consulta.outline.functions[f.0 as usize].parameters.iter() {
            let Some(n) = q.externo.or(q.name) else { continue };
            let nome = consulta.nome(n).to_string();
            let posicional = q.kind != ast::ParameterKind::Named;
            if usados.contains(&nome) || (posicional != (prm.kind != ast::ParameterKind::Named)) {
                continue;
            }
            saida.push((nome, Some(consulta.formatar(q.ty))));
        }
    }
    Some(saida)
}

/// A biblioteca alvo do `import`/`export` cujo `show`/`hide` tem um nome
/// começando em `inicio`.
fn biblioteca_do_combinador(consulta: &Consulta, unidade: dartforge_elements::model::UnitId, inicio: usize) -> Option<LibraryId> {
    let u = consulta.programa.unit(unidade);
    let lib = consulta.programa.library(u.library);
    let alvos = lib
        .imports
        .iter()
        .filter(|i| i.unit == unidade)
        .map(|i| (i.directive, i.library))
        .chain(lib.exports.iter().filter(|e| e.unit == unidade).map(|e| (e.directive, e.library)));
    for (indice, alvo) in alvos {
        let Some(d) = u.unit.directives.get(indice) else { continue };
        let combinadores = match &d.kind {
            ast::DirectiveKind::Import { combinators, .. } | ast::DirectiveKind::Export { combinators, .. } => combinators,
            _ => continue,
        };
        for c in combinadores {
            let (ast::Combinator::Show(nomes) | ast::Combinator::Hide(nomes)) = c;
            if nomes.iter().any(|n| n.span.start == inicio) {
                return Some(alvo);
            }
        }
    }
    None
}

/// O sentinela em `inicio` é o nome de uma declaração: variável local ou
/// de topo, ou campo, com `var`/`final`/`const`/tipo escrito; parâmetro;
/// função, método, classe, mixin, enum, extensão, typedef, constante de
/// enum; parâmetro de tipo.
fn nome_declarado(ast: &ast::Ast, inicio: usize) -> bool {
    let eh = |n: &ast::Name| n.span.start == inicio;
    let lista = |l: &ast::VariableList| (l.var_ || l.final_ || l.const_ || l.ty.is_some()) && l.variables.iter().any(|v| eh(&v.name));
    ast.stmts.iter().any(|s| match &s.kind {
        StmtKind::Variables(l) => lista(l),
        StmtKind::ForIn { target: ast::ForInTarget::Declared { name, .. }, .. } => eh(name),
        _ => false,
    }) || ast.decls.iter().any(|d| match &d.kind {
        ast::DeclKind::Variables(l) => lista(l),
        ast::DeclKind::Class(k) => eh(&k.name),
        ast::DeclKind::Mixin(k) => eh(&k.name),
        ast::DeclKind::Enum(k) => eh(&k.name) || k.constants.iter().any(|c| eh(&c.name)),
        ast::DeclKind::Extension(k) => k.name.as_ref().is_some_and(eh),
        ast::DeclKind::ExtensionType(k) => eh(&k.name),
        ast::DeclKind::Typedef(k) => eh(&k.name),
        ast::DeclKind::Function(_) => false,
    }) || ast.members.iter().any(|m| match &m.kind {
        ast::MemberKind::Field(l) => lista(l),
        _ => false,
    }) || ast.functions.iter().any(|f| {
        f.name.as_ref().is_some_and(eh)
            || f.parameters.iter().flatten().any(|p| p.ty.is_some() && p.name.as_ref().is_some_and(eh))
            || f.type_params.iter().any(|t| eh(&t.name))
    })
}

/// O cursor está dentro de comentário ou do texto de uma string: não há o
/// que completar. Interpolações (`$nome`, `${…}`) são código.
fn fora_de_codigo(texto: &str, offset: usize) -> bool {
    let Ok(tokens) = dartforge_frontend::lexer::lex(texto) else {
        return false;
    };
    let mut fim_anterior = 0;
    for t in &tokens {
        if t.span.start >= offset {
            break;
        }
        if offset < t.span.end {
            // Dentro de um token: só strings são "fora de código".
            return !matches!(
                t.kind,
                dartforge_frontend::token::Kind::Ident
                    | dartforge_frontend::token::Kind::Keyword(_)
            );
        }
        fim_anterior = t.span.end;
    }
    let lacuna = &texto[fim_anterior.min(offset)..offset];
    if let Some(i) = lacuna.find("//") {
        return !lacuna[i..].contains('\n');
    }
    lacuna.contains("/*")
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
        });
        self.itens.last_mut().expect("item recém-empurrado")
    }

    fn palavras(&mut self, palavras: &[&'static str]) {
        for p in palavras {
            self.empurrar(
                grupo::PALAVRA,
                especie::PALAVRA_CHAVE,
                p.to_string(),
                p.to_string(),
                None,
            )
            .rel
            .palavra = Some(p);
        }
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
        if let Some(item) = self.itens.last_mut() {
            item.rel.especie = Some(especie);
            item.rel.tipo = tipo_do_item;
            item.rel.distancia = distancia;
            item.rel.comeca_com_dolar = nome.starts_with('$');
            item.rel.no_such_method = nome == "noSuchMethod";
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
                let item = self.empurrar(grupo, especie, nome.clone(), nome, None);
                item.origem = origem;
                item.rel.especie = Some(especie_rel);
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
                    let item = self.empurrar(grupo, especie::VARIAVEL, nome.clone(), nome, detalhe);
                    item.origem = origem;
                    item.rel.especie = Some(crate::relevancia::Especie::VariavelDeTopo);
                    item.rel.tipo = consulta.tipo_da_variavel(v);
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

    fn espaco_de_nomes(
        &mut self,
        consulta: &Consulta,
        espaco: &Namespace,
        grupo_de: impl Fn(LibraryId) -> u8,
    ) {
        let mut entradas: Vec<(&SymbolId, &dartforge_elements::model::Binding)> =
            espaco.iter().collect();
        entradas.sort_by_key(|(s, _)| consulta.nome(**s).to_string());
        for (_, vinculo) in entradas {
            if vinculo.ambiguous {
                continue;
            }
            let Some(elemento) = vinculo.getter.or(vinculo.setter) else {
                continue;
            };
            let dona = biblioteca_do_elemento(consulta, elemento);
            self.elemento(consulta, grupo_de(dona), elemento);
        }
    }

    /// Declarações de topo, importados sem prefixo e os prefixos.
    fn biblioteca(&mut self, consulta: &Consulta) {
        let lib = consulta.programa.library(self.biblioteca);
        let propria = self.biblioteca;
        self.espaco_de_nomes(consulta, &lib.scope, |dona| {
            if dona == propria {
                grupo::BIBLIOTECA
            } else {
                grupo::IMPORTADO
            }
        });
        let mut prefixos: Vec<String> = lib
            .prefixes
            .keys()
            .map(|p| consulta.nome(*p).to_string())
            .collect();
        prefixos.sort();
        for p in prefixos {
            self.empurrar(grupo::PREFIXO, especie::MODULO, p.clone(), p, None)
                .rel
                .especie = Some(crate::relevancia::Especie::Prefixo);
        }
    }

    /// Posição de tipo: só nomes de tipo (do escopo da biblioteca ou do
    /// prefixo), os parâmetros de tipo em escopo, os prefixos e
    /// `dynamic`/`void`.
    fn tipos(
        &mut self,
        consulta: &Consulta,
        unidade: dartforge_elements::model::UnitId,
        inicio: usize,
        prefixo: Option<SymbolId>,
    ) {
        let lib = consulta.programa.library(self.biblioteca);
        let espaco = match prefixo {
            Some(p) => match lib.prefixes.get(&p) {
                Some(e) => e,
                None => return,
            },
            None => &lib.scope,
        };
        let mut entradas: Vec<(&SymbolId, &dartforge_elements::model::Binding)> =
            espaco.iter().collect();
        entradas.sort_by_key(|(s, _)| consulta.nome(**s).to_string());
        let propria = self.biblioteca;
        for (_, vinculo) in entradas {
            if vinculo.ambiguous {
                continue;
            }
            if let Some(el @ (Element::Class(_) | Element::Typedef(_))) = vinculo.getter {
                let dona = biblioteca_do_elemento(consulta, el);
                let g = if dona == propria {
                    grupo::BIBLIOTECA
                } else {
                    grupo::IMPORTADO
                };
                self.elemento(consulta, g, el);
            }
        }
        if prefixo.is_some() {
            return;
        }
        let ast = &consulta.programa.unit(unidade).ast;
        for nome in parametros_de_tipo_em(ast, inicio) {
            let nome = consulta.nome(nome).to_string();
            self.empurrar(
                grupo::LOCAL,
                especie::PARAMETRO_DE_TIPO,
                nome.clone(),
                nome,
                None,
            );
        }
        let mut prefixos: Vec<String> = lib
            .prefixes
            .keys()
            .map(|p| consulta.nome(*p).to_string())
            .collect();
        prefixos.sort();
        for p in prefixos {
            self.empurrar(grupo::PREFIXO, especie::MODULO, p.clone(), p, None);
        }
        self.palavras(&["dynamic", "void"]);
    }

    /// O `NotImportedCompletionPass` (docs/LSP-ESPECIFICACAO.md §14.9) com
    /// a `StaticMembersOperation`: para cada biblioteca conhecida que o
    /// `FileStateFilter` aceita (na ordem de `conhecidas::candidatas`), que
    /// não é a do pedido nem está importada sem combinadores pela unidade
    /// definidora (`_ImportSummary`), as declarações do `exportNamespace`
    /// (`_addExternalTopLevelDeclarations`: só tipos com `mustBeType`), cada
    /// uma com o `import` que a torna visível. O `VisibilityTracker` esconde
    /// as de nome já sugerido pelo escopo; o filtro pelo digitado é o do
    /// coletor. Não corta: o `isIncomplete` do Dart vem só do orçamento.
    #[allow(clippy::too_many_arguments)]
    fn nao_importados(
        &mut self,
        consulta: &Consulta,
        unidade: dartforge_elements::model::UnitId,
        texto: &str,
        so_tipos: bool,
        sdk: &SdkLayout,
        indices: Indices<'_>,
        documentos: &DocumentStore,
    ) -> bool {
        use crate::conhecidas::Especie as E;
        let programa = &consulta.programa;
        let lib = programa.library(self.biblioteca);
        let Some(arquivo) = programa.unit(unidade).path.clone() else {
            return false;
        };
        let declarados: HashSet<String> = lib
            .scope
            .keys()
            .map(|s| consulta.nome(*s).to_string())
            .chain(self.itens.iter().map(|i| i.inserir.clone()))
            .collect();
        let definidora = lib.units.first().copied();
        let chave = |c: &std::path::Path| dartforge_elements::gerado::chave(c);
        let importadas: HashSet<PathBuf> = lib
            .imports
            .iter()
            .filter(|i| Some(i.unit) == definidora && i.combinators.is_empty())
            .filter_map(|i| programa.library(i.library).units.first())
            .filter_map(|u| programa.unit(*u).path.as_deref().map(chave))
            .collect();
        let proprias: HashSet<PathBuf> = lib
            .units
            .iter()
            .filter_map(|u| programa.unit(*u).path.as_deref().map(chave))
            .collect();
        let resolvedor = crate::conhecidas::Resolvedor {
            sdk: Some(sdk),
            pacotes: dartforge_elements::config::PackageConfig::discover(&arquivo)
                .and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()),
        };
        let raiz = crate::projeto::raiz_do_projeto(&arquivo);
        let pacote = crate::indice::nome_do_pacote(&raiz);
        let unit = &programa.unit(unidade).unit;
        let conhecidas = indices.conhecidas;
        for candidata in crate::conhecidas::candidatas(&arquivo, Some(sdk), conhecidas, documentos) {
            let k = chave(&candidata.caminho);
            if proprias.contains(&k) || importadas.contains(&k) {
                continue;
            }
            // O texto da URI no `import`: `dart:`/`package:` como estão; um
            // arquivo do projeto fora de `lib/`, relativo.
            let uri = if candidata.sdk || candidata.uri.starts_with("package:") {
                candidata.uri.clone()
            } else {
                match crate::indice::uri_de_import(&arquivo, &candidata.caminho, &raiz, pacote.as_deref()) {
                    Some(u) => u,
                    None => continue,
                }
            };
            for (nome, origem, d) in conhecidas.exportados(&candidata.caminho, &resolvedor, documentos) {
                let tipo = crate::conhecidas::DE_TIPO.contains(&d.especie);
                if (so_tipos && !tipo) || declarados.contains(&nome) {
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
                let (span, novo) = crate::acoes::inserir_import(texto, unit, &uri);
                let item = self.empurrar(
                    grupo::NAO_IMPORTADO,
                    especie_lsp,
                    rotulo,
                    nome.clone(),
                    Some(format!("Auto import from '{uri}'")),
                );
                item.importar = Some(ImportAutomatico { uri: uri.clone(), span, texto: novo });
                item.origem = Some((origem, d.inicio));
                item.rel.nao_importado = true;
                item.rel.especie = Some(rel);
                if funcao {
                    item.chamada = Some(if d.sem_parametros { Chamada::Parametros(Vec::new()) } else { Chamada::Desconhecida });
                }
            }
        }
        false
    }

    /// Nomes visíveis no identificador sondado.
    fn escopo(&mut self, consulta: &mut Consulta, unidade: dartforge_elements::model::UnitId) {
        if let Some(escopo) = consulta.escopo.clone() {
            let ast = &consulta.programa.unit(unidade).ast;
            let eh_parametro = |o: usize| {
                ast.functions.iter().flat_map(|f| f.parameters.iter().flatten()).any(|p| p.name.is_some_and(|n| n.span.start == o))
                    || ast.members.iter().any(|m| matches!(&m.kind, ast::MemberKind::Constructor(k) if k.parameters.iter().any(|p| p.name.is_some_and(|n| n.span.start == o))))
            };
            for (i, local) in escopo.locais.iter().enumerate() {
                let nome = consulta.nome(local.nome).to_string();
                let especie = if local.funcao {
                    especie::FUNCAO
                } else {
                    especie::VARIAVEL
                };
                let detalhe = consulta.formatar(local.tipo);
                let parametro = eh_parametro(local.offset);
                let item = self.empurrar(grupo::LOCAL, especie, nome.clone(), nome, Some(detalhe));
                if local.funcao {
                    item.rel.especie = Some(crate::relevancia::Especie::Funcao);
                    item.rel.tipo = retorno(consulta, local.tipo);
                } else {
                    item.rel.especie = Some(if parametro { crate::relevancia::Especie::Parametro } else { crate::relevancia::Especie::Local });
                    item.rel.tipo = Some(local.tipo);
                    item.rel.local = true;
                    item.rel.distancia = Some(i as u32);
                }
            }
            for (nome, _) in &escopo.parametros_de_tipo {
                let nome = consulta.nome(*nome).to_string();
                self.empurrar(
                    grupo::LOCAL,
                    especie::PARAMETRO_DE_TIPO,
                    nome.clone(),
                    nome,
                    None,
                )
                .rel
                .especie = Some(crate::relevancia::Especie::ParametroDeTipo);
            }
            if let Some(classe) = escopo.classe {
                if !escopo.estatico
                    && let Some(this) = escopo.tipo_this
                {
                    self.membros_de_instancia(consulta, this);
                }
                self.estaticos_da_classe(consulta, classe, false);
            }
            if let Some(extensao) = escopo.extensao {
                let x = consulta.programa.extension(extensao);
                let mut membros: Vec<FunctionElementId> =
                    x.static_members.values().copied().collect();
                if !escopo.estatico {
                    membros.extend(x.instance_members.values().copied());
                }
                membros.sort();
                for f in membros {
                    let tipo = tipo_declarado(consulta, f);
                    self.funcao(consulta, grupo::MEMBRO, f, tipo);
                }
            }
        }
        self.biblioteca(consulta);
    }

    /// Membros estáticos (e, com `construtores`, os construtores nomeados)
    /// de uma classe.
    fn estaticos_da_classe(&mut self, consulta: &Consulta, classe: ClassId, construtores: bool) {
        let c = consulta.programa.class(classe);
        // Constantes de enum (`Cor.▮` lista `azul`, `verde`…), na ordem
        // declarada, com o tipo do enum.
        for v in c.enum_constants.clone() {
            let nome = consulta.nome(consulta.programa.variable(v).name).to_string();
            let tipo = consulta.tipo_da_variavel(v).map(|t| consulta.formatar(t));
            let origem = consulta.origem(consulta.inicio_da_variavel(v));
            let tipo_id = consulta.tipo_da_variavel(v);
            let item = self.empurrar(grupo::MEMBRO, especie::MEMBRO_DE_ENUM, nome.clone(), nome, tipo);
            item.origem = origem;
            item.rel.especie = Some(crate::relevancia::Especie::SemTabela);
            item.rel.tipo = tipo_id;
        }
        let c = consulta.programa.class(classe);
        let mut membros: Vec<FunctionElementId> = c.static_members.values().copied().collect();
        membros.sort();
        for f in membros {
            // O acessor de uma constante de enum já entrou acima.
            if consulta.programa.function(f).variable.is_some_and(|v| c.enum_constants.contains(&v)) {
                continue;
            }
            let tipo = tipo_declarado(consulta, f);
            self.funcao(consulta, grupo::MEMBRO, f, tipo);
        }
        if construtores {
            for (nome, f) in c.construtores() {
                if consulta.nome(nome).is_empty() {
                    continue;
                }
                let tipo = consulta.outline.functions[f.0 as usize].signature;
                self.funcao(consulta, grupo::MEMBRO, f, tipo);
            }
        }
    }

    /// Membros de instância de um receptor com tipo estático `receptor`,
    /// com os tipos vistos por ele (substituídos), pela busca de membros de
    /// `crates/types`. Inclui os de extensões aplicáveis.
    fn membros_de_instancia(&mut self, consulta: &mut Consulta, receptor: TypeId) {
        let mut busca = receptor;
        // Parâmetro de tipo: os membros do limite.
        for _ in 0..16 {
            match consulta.tabela.get(busca).clone() {
                Type::TypeParameter { param, .. } => busca = consulta.tabela.param(param).bound,
                Type::Intersection { bound, .. } => busca = bound,
                Type::Dynamic | Type::Never | Type::Void | Type::FutureOr { .. } | Type::Null => {
                    busca = consulta.core.object;
                    break;
                }
                _ => break,
            }
        }
        let mut classes = Vec::new();
        self.receptor = None;
        match consulta.tabela.get(busca).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                self.receptor = Some(class);
                classes.push(class)
            }
            Type::Record {
                positional, named, ..
            } => {
                for (i, t) in positional.iter().enumerate() {
                    let nome = format!("${}", i + 1);
                    self.empurrar(
                        grupo::MEMBRO,
                        especie::CAMPO,
                        nome.clone(),
                        nome,
                        Some(consulta.formatar(*t)),
                    );
                }
                for (n, t) in named.iter() {
                    let nome = consulta.nome(*n).to_string();
                    self.empurrar(
                        grupo::MEMBRO,
                        especie::CAMPO,
                        nome.clone(),
                        nome,
                        Some(consulta.formatar(*t)),
                    );
                }
                classes.extend(consulta.core.record_class);
            }
            _ => classes.extend(consulta.core.object_class),
        }
        let mut nomes: Vec<SymbolId> = Vec::new();
        let mut vistos_nomes = HashSet::new();
        let mut vistas = HashSet::new();
        while let Some(c) = classes.pop() {
            if !vistas.insert(c) {
                continue;
            }
            let classe = consulta.programa.class(c);
            let mut proprios: Vec<SymbolId> = classe.instance_members.keys().copied().collect();
            proprios.sort_by_key(|s| s.as_u32());
            for n in proprios {
                if vistos_nomes.insert(n) {
                    nomes.push(n);
                }
            }
            classes.extend(classe.supertype_class);
            classes.extend(classe.mixin_classes.iter().copied());
            classes.extend(classe.interface_classes.iter().copied());
            classes.extend(classe.on_classes.iter().copied());
        }
        // Extensões visíveis na biblioteca (as mesmas que a busca considera).
        let lib = consulta.programa.library(self.biblioteca);
        let mut extensoes: Vec<_> = lib
            .scope
            .values()
            .filter_map(|b| match b.getter {
                Some(Element::Extension(x)) => Some(x),
                _ => None,
            })
            .collect();
        extensoes.sort();
        for x in extensoes {
            let mut proprios: Vec<SymbolId> = consulta
                .programa
                .extension(x)
                .instance_members
                .keys()
                .copied()
                .collect();
            proprios.sort_by_key(|s| s.as_u32());
            for n in proprios {
                if vistos_nomes.insert(n) {
                    nomes.push(n);
                }
            }
        }
        for nome in nomes {
            let texto = consulta.nome(nome);
            if texto.ends_with('=')
                || !texto.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
            {
                continue;
            }
            let biblioteca = self.biblioteca;
            let achado = consulta
                .resolvedor()
                .lookup_member(busca, nome, false, biblioteca);
            let Some((resolvido, tipo)) = achado else {
                continue;
            };
            let funcao = match resolvido {
                Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                } => f,
                Resolved::ExtensionMember { member, .. } => member,
                Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                } => match consulta.programa.variable(v).getter {
                    Some(g) => g,
                    None => continue,
                },
                _ => continue,
            };
            let de_object = consulta.core.object_class.is_some()
                && consulta.programa.function(funcao).class == consulta.core.object_class;
            let grupo = if de_object {
                grupo::MEMBRO_DE_OBJECT
            } else {
                grupo::MEMBRO
            };
            self.funcao(consulta, grupo, funcao, tipo);
        }
    }

    /// `alvo.▮`: membros de instância pelo tipo do alvo, estáticos e
    /// construtores de uma classe, ou os nomes de um prefixo de import.
    fn membros_do_alvo(
        &mut self,
        consulta: &mut Consulta,
        unidade: dartforge_elements::model::UnitId,
        _expr: ExprId,
        alvo: ExprId,
    ) {
        let corpos = &consulta.corpos.units[unidade.0 as usize];
        let resolvido = corpos.get_resolved(alvo).cloned();
        let ast = &consulta.programa.unit(unidade).ast;
        if let (Some(Resolved::Prefix(_)), ExprKind::Identifier(p)) =
            (&resolvido, &ast.expr(alvo).kind)
        {
            let lib = consulta.programa.library(self.biblioteca);
            if let Some(espaco) = lib.prefixes.get(&p.sym) {
                let propria = self.biblioteca;
                self.espaco_de_nomes(consulta, espaco, |dona| {
                    if dona == propria {
                        grupo::BIBLIOTECA
                    } else {
                        grupo::IMPORTADO
                    }
                });
            }
            return;
        }
        match resolvido {
            Some(Resolved::Element(Element::Class(c))) => {
                self.estaticos_da_classe(consulta, c, true);
                return;
            }
            Some(Resolved::Element(Element::Extension(x))) => {
                let mut membros: Vec<FunctionElementId> = consulta
                    .programa
                    .extension(x)
                    .static_members
                    .values()
                    .copied()
                    .collect();
                membros.sort();
                for f in membros {
                    let tipo = tipo_declarado(consulta, f);
                    self.funcao(consulta, grupo::MEMBRO, f, tipo);
                }
                return;
            }
            _ => {}
        }
        let Some(tipo) = consulta.corpos.units[unidade.0 as usize].get_type(alvo) else {
            return;
        };
        self.membros_de_instancia(consulta, tipo);
    }

    /// Argumento posicional sendo digitado numa chamada: os parâmetros
    /// nomeados ainda não passados.
    fn argumentos_nomeados(
        &mut self,
        consulta: &Consulta,
        unidade: dartforge_elements::model::UnitId,
        expr: ExprId,
    ) {
        let ast = &consulta.programa.unit(unidade).ast;
        let corpos = &consulta.corpos.units[unidade.0 as usize];
        for (i, e) in ast.exprs.iter().enumerate() {
            let (argumentos, alvo) = match &e.kind {
                ExprKind::Call { target, arguments } => (arguments, Some(*target)),
                ExprKind::InstanceCreation { arguments, .. } => (arguments, None),
                _ => continue,
            };
            if !argumentos
                .args
                .iter()
                .any(|a| a.name.is_none() && a.value == expr)
            {
                continue;
            }
            let passados: HashSet<SymbolId> = argumentos
                .args
                .iter()
                .filter_map(|a| a.name.map(|n| n.sym))
                .collect();
            let mut nomeados: Vec<(String, TypeId, bool)> = Vec::new();
            let construtor = match corpos.get_resolved(ExprId(i as u32)) {
                Some(Resolved::Constructor(f)) => Some(*f),
                _ => alvo.and_then(|a| match corpos.get_resolved(a) {
                    Some(Resolved::Constructor(f)) => Some(*f),
                    _ => None,
                }),
            };
            if let Some(f) = construtor {
                for p in consulta.outline.functions[f.0 as usize].parameters.iter() {
                    if p.kind == ast::ParameterKind::Named
                        && let Some(n) = p.externo.or(p.name).filter(|n| !passados.contains(n))
                    {
                        nomeados.push((consulta.nome(n).to_string(), p.ty, p.required));
                    }
                }
            } else if let Some(t) = alvo.and_then(|a| corpos.get_type(a))
                && let Type::Function { named, .. } = consulta.tabela.get(t)
            {
                for (n, t, obrigatorio) in named.iter() {
                    if !passados.contains(n) {
                        nomeados.push((consulta.nome(*n).to_string(), *t, *obrigatorio));
                    }
                }
            }
            for (nome, tipo, obrigatorio) in nomeados {
                self.empurrar(
                    grupo::NOMEADO,
                    especie::VARIAVEL,
                    format!("{nome}: "),
                    format!("{nome}: "),
                    Some(consulta.formatar(tipo)),
                )
                .rel
                .fixa = Some(if obrigatorio { 950 } else { 900 });
            }
            return;
        }
    }
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

/// Os parâmetros de tipo em escopo em `offset`: os das declarações, funções
/// e tipos de função que o contêm, do mais interno para o mais externo.
fn parametros_de_tipo_em(ast: &ast::Ast, offset: usize) -> Vec<SymbolId> {
    let dentro = |s: Span| s.start <= offset && offset < s.end;
    let mut escopos: Vec<(usize, &[ast::TypeParameter])> = Vec::new();
    for d in &ast.decls {
        let ps: &[ast::TypeParameter] = match &d.kind {
            ast::DeclKind::Class(c) => &c.type_params,
            ast::DeclKind::Mixin(m) => &m.type_params,
            ast::DeclKind::Enum(e) => &e.type_params,
            ast::DeclKind::Extension(x) => &x.type_params,
            ast::DeclKind::ExtensionType(x) => &x.type_params,
            ast::DeclKind::Typedef(t) => &t.type_params,
            _ => continue,
        };
        if dentro(d.span) {
            escopos.push((d.span.end - d.span.start, ps));
        }
    }
    for f in &ast.functions {
        if dentro(f.span) {
            escopos.push((f.span.end - f.span.start, &f.type_params));
        }
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind
            && dentro(t.span)
        {
            escopos.push((t.span.end - t.span.start, type_params));
        }
    }
    escopos.sort_by_key(|(tam, _)| *tam);
    let mut vistos = HashSet::new();
    escopos
        .into_iter()
        .flat_map(|(_, ps)| ps.iter().map(|t| t.name.sym))
        .filter(|s| vistos.insert(*s))
        .collect()
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

/// O tipo que a posição da expressão `expr` espera (`computeContextType`
/// do Dart, nos casos comuns): o parâmetro do argumento, o alvo da
/// atribuição, o tipo da variável com tipo escrito, o retorno da função, o
/// `bool` de uma condição. `dynamic` não conta.
fn tipo_esperado(consulta: &mut Consulta, unidade: dartforge_elements::model::UnitId, expr: ExprId) -> Option<TypeId> {
    let ast = &consulta.programa.unit(unidade).ast;
    let corpos = &consulta.corpos.units[unidade.0 as usize];
    let mut esperado: Option<TypeId> = None;
    for s in &ast.stmts {
        match &s.kind {
            StmtKind::If { condition, .. } | StmtKind::While { condition, .. } | StmtKind::DoWhile { condition, .. } if *condition == expr => {
                esperado = Some(consulta.core.bool_);
            }
            StmtKind::Variables(l) if l.ty.is_some() => {
                if let Some(v) = l.variables.iter().find(|v| v.initializer == Some(expr)) {
                    esperado = corpos.tipo_local(v.name.span.start);
                }
            }
            StmtKind::Return(Some(e)) if *e == expr => {
                // O retorno escrito da função que contém o comando.
                let f = ast.functions.iter().filter(|f| f.span.start <= s.span.start && s.span.end <= f.span.end).min_by_key(|f| f.span.end - f.span.start);
                if let Some(f) = f
                    && let Some(n) = f.name
                {
                    esperado = corpos.tipo_local(n.span.start).and_then(|t| retorno(consulta, t)).or_else(|| {
                        consulta
                            .programa
                            .functions
                            .iter()
                            .position(|x| matches!(x.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == unidade && ast.function(function).span == f.span))
                            .map(|i| consulta.outline.functions[i].return_type)
                    });
                }
            }
            _ => {}
        }
    }
    if esperado.is_none() {
        for (i, e) in ast.exprs.iter().enumerate() {
            match &e.kind {
                ExprKind::Assign { target, value, .. } if *value == expr => esperado = corpos.get_type(*target),
                ExprKind::Call { target, arguments } => {
                    if let Some(k) = arguments.args.iter().position(|a| a.value == expr) {
                        let a = &arguments.args[k];
                        let construtor = match corpos.get_resolved(ExprId(i as u32)) {
                            Some(Resolved::Constructor(f)) => Some(*f),
                            _ => None,
                        };
                        let tipo_f = construtor.map(|f| consulta.outline.functions[f.0 as usize].signature).or_else(|| corpos.get_type(*target));
                        esperado = tipo_f.and_then(|t| parametro_do_argumento(consulta, t, arguments, k, a.name.map(|n| n.sym)));
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => {
                    if let Some(k) = arguments.args.iter().position(|a| a.value == expr) {
                        let a = &arguments.args[k];
                        if let Some(Resolved::Constructor(f)) = corpos.get_resolved(ExprId(i as u32)) {
                            let t = consulta.outline.functions[f.0 as usize].signature;
                            esperado = parametro_do_argumento(consulta, t, arguments, k, a.name.map(|n| n.sym));
                        }
                    }
                }
                _ => {}
            }
            if esperado.is_some() {
                break;
            }
        }
    }
    esperado.filter(|t| !matches!(consulta.tabela.get(*t), Type::Dynamic))
}

/// O tipo do parâmetro que recebe o `k`-ésimo argumento (posicional pela
/// posição entre os posicionais, nomeado pelo nome).
fn parametro_do_argumento(consulta: &Consulta, tipo_f: TypeId, argumentos: &ast::Arguments, k: usize, nome: Option<SymbolId>) -> Option<TypeId> {
    let Type::Function { positional, optional, named, .. } = consulta.tabela.get(tipo_f) else { return None };
    match nome {
        Some(n) => named.iter().find(|(s, _, _)| *s == n).map(|(_, t, _)| *t),
        None => {
            let posicao = argumentos.args[..k].iter().filter(|a| a.name.is_none()).count();
            positional.iter().chain(optional.iter()).nth(posicao).copied()
        }
    }
}

/// O local do completar num nome de tipo (`VariableDeclarationList_type`,
/// `FormalParameterList_parameter`, `TypeArgumentList_argument`…).
fn local_do_tipo(ast: &ast::Ast, inicio: usize) -> Option<String> {
    let tipo = ast.types.iter().enumerate().find(|(_, t)| match &t.kind {
        ast::TypeKind::Named { name, .. } => name.last().is_some_and(|n| n.span.start == inicio),
        _ => false,
    })?;
    let id = ast::TypeId(tipo.0 as u32);
    let em_lista = |l: &ast::VariableList| l.ty == Some(id);
    if ast.stmts.iter().any(|s| matches!(&s.kind, StmtKind::Variables(l) if em_lista(l))) {
        return Some("VariableDeclarationList_type".into());
    }
    if ast.members.iter().any(|m| matches!(&m.kind, ast::MemberKind::Field(l) if em_lista(l))) {
        return Some("FieldDeclaration_fields".into());
    }
    if ast.functions.iter().any(|f| f.parameters.iter().flatten().any(|p| p.ty == Some(id))) {
        return Some("FormalParameterList_parameter".into());
    }
    if ast.functions.iter().any(|f| f.return_type == Some(id)) {
        let metodo = ast.members.iter().any(|m| matches!(m.kind, ast::MemberKind::Method(f) if ast.function(f).return_type == Some(id)));
        return Some(if metodo { "MethodDeclaration_returnType" } else { "FunctionDeclaration_returnType" }.into());
    }
    if ast.types.iter().any(|t| matches!(&t.kind, ast::TypeKind::Named { args, .. } if args.contains(&id))) {
        return Some("TypeArgumentList_argument".into());
    }
    for e in &ast.exprs {
        match &e.kind {
            ExprKind::Is { ty, .. } if *ty == id => return Some("IsExpression_type".into()),
            ExprKind::As { ty, .. } if *ty == id => return Some("AsExpression_type".into()),
            _ => {}
        }
    }
    for d in &ast.decls {
        if let ast::DeclKind::Class(k) = &d.kind {
            if k.extends == Some(id) {
                return Some("ExtendsClause_superclass".into());
            }
            if k.implements.contains(&id) {
                return Some("ImplementsClause_interface".into());
            }
            if k.with.contains(&id) {
                return Some("WithClause_mixinType".into());
            }
        }
    }
    None
}
