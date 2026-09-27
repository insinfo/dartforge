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
use std::collections::HashSet;
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

/// Resultado do completar: o intervalo do prefixo (bytes) e os itens em ordem.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completar {
    pub inicio: usize,
    pub fim: usize,
    pub itens: Vec<ItemCompletar>,
    /// A lista foi cortada (nomes não importados acima do teto).
    pub incompleta: bool,
}

/// Onde o sentinela caiu na análise.
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
const TETO_NAO_IMPORTADOS: usize = 200;

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
pub(crate) fn completar(
    sdk: &SdkLayout,
    indices: Indices<'_>,
    documentos: &DocumentStore,
    uri: &str,
    texto: &str,
    offset: usize,
    features: LibraryFeatures,
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
    };

    let sentinela = preparado
        .as_ref()
        .and_then(|_| achar_sentinela(&consulta.programa.unit(unidade).ast, inicio));
    // Nomes de bibliotecas não importadas cabem onde um nome solto cabe.
    let mut nao_importados: Option<bool> = None;
    match sentinela {
        Some(Sentinela::Expr(expr, Some(alvo))) => {
            coletor.membros_do_alvo(&mut consulta, unidade, expr, alvo)
        }
        Some(Sentinela::Expr(expr, None)) => {
            coletor.argumentos_nomeados(&consulta, unidade, expr);
            coletor.escopo(&mut consulta);
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
            &consulta, unidade, texto, digitado, so_tipos, indices, documentos,
        );
    }

    let mut itens = coletor.itens;
    itens.retain_mut(|i| {
        if i.inserir == SENTINELA {
            return false;
        }
        match crate::aproximado::pontuar(digitado, i.inserir.trim_end_matches([':', ' '])) {
            Some(q) => {
                i.qualidade = u8::from(q > 1);
                true
            }
            None => false,
        }
    });
    itens.sort_by(|a, b| {
        (
            a.qualidade,
            a.grupo,
            a.inserir.to_ascii_lowercase(),
            &a.inserir,
        )
            .cmp(&(
                b.qualidade,
                b.grupo,
                b.inserir.to_ascii_lowercase(),
                &b.inserir,
            ))
    });
    // Um nome aparece uma vez; o não importado de bibliotecas diferentes,
    // uma vez por biblioteca.
    let mut vistos = HashSet::new();
    itens.retain(|i| {
        vistos.insert((
            i.inserir.clone(),
            i.importar.as_ref().map(|x| x.uri.clone()),
        ))
    });
    Some(Completar {
        inicio,
        fim: offset,
        itens,
        incompleta,
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
        });
        self.itens.last_mut().expect("item recém-empurrado")
    }

    fn palavras(&mut self, palavras: &[&str]) {
        for p in palavras {
            self.empurrar(
                grupo::PALAVRA,
                especie::PALAVRA_CHAVE,
                p.to_string(),
                p.to_string(),
                None,
            );
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
                let origem = consulta.origem(consulta.inicio_do_elemento(elemento));
                self.empurrar(grupo, especie, nome.clone(), nome, None)
                    .origem = origem;
            }
            Element::Typedef(t) => {
                let nome = consulta.nome(programa.typedef(t).name).to_string();
                if !self.invisivel(consulta, &nome, programa.typedef(t).library) {
                    let origem = consulta.origem(consulta.inicio_do_elemento(elemento));
                    self.empurrar(grupo, especie::CLASSE, nome.clone(), nome, None)
                        .origem = origem;
                }
            }
            Element::Extension(x) => {
                let Some(n) = programa.extension(x).name else {
                    return;
                };
                let nome = consulta.nome(n).to_string();
                if !self.invisivel(consulta, &nome, programa.extension(x).library) {
                    let origem = consulta.origem(consulta.inicio_do_elemento(elemento));
                    self.empurrar(grupo, especie::CLASSE, nome.clone(), nome, None)
                        .origem = origem;
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
                    self.empurrar(grupo, especie::VARIAVEL, nome.clone(), nome, detalhe)
                        .origem = origem;
                }
            }
            Element::Prefix(_, p) => {
                let nome = consulta.nome(p).to_string();
                self.empurrar(grupo::PREFIXO, especie::MODULO, nome.clone(), nome, None);
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
            self.empurrar(grupo::PREFIXO, especie::MODULO, p.clone(), p, None);
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

    /// Nomes públicos de bibliotecas ainda não importadas (SDK e projeto)
    /// que casam com o digitado e não estão visíveis, cada um com o `import`
    /// que o torna visível. Devolve se a lista foi cortada no teto.
    #[allow(clippy::too_many_arguments)]
    fn nao_importados(
        &mut self,
        consulta: &Consulta,
        unidade: dartforge_elements::model::UnitId,
        texto: &str,
        digitado: &str,
        so_tipos: bool,
        indices: Indices<'_>,
        documentos: &DocumentStore,
    ) -> bool {
        let programa = &consulta.programa;
        let lib = programa.library(self.biblioteca);
        let Some(arquivo) = programa.unit(unidade).path.clone() else {
            return false;
        };
        let visivel = |nome: &str| lib.scope.keys().any(|s| consulta.nome(*s) == nome);
        let importadas: HashSet<String> = lib
            .imports
            .iter()
            .map(|i| programa.library(i.library).uri.clone())
            .collect();
        let mut candidatos: Vec<(u8, String, crate::indice::Declarado)> = Vec::new();
        for (nome, por_uri) in &indices.sdk.por_nome {
            let Some(q) = crate::aproximado::pontuar(digitado, nome) else {
                continue;
            };
            if visivel(nome) {
                continue;
            }
            for (uri, d) in por_uri {
                if uri != "dart:core" && !importadas.contains(uri) && (!so_tipos || d.tipo) {
                    candidatos.push((q, uri.clone(), d.clone()));
                }
            }
        }
        let raiz = crate::projeto::raiz_do_projeto(&arquivo);
        let pacote = crate::indice::nome_do_pacote(&raiz);
        let proprias: HashSet<PathBuf> = lib
            .units
            .iter()
            .filter_map(|u| programa.unit(*u).path.clone())
            .collect();
        for (caminho, nomes) in indices.projeto.atualizar(&raiz, documentos) {
            if proprias.contains(caminho) {
                continue;
            }
            let Some(uri) =
                crate::indice::uri_de_import(&arquivo, caminho, &raiz, pacote.as_deref())
            else {
                continue;
            };
            let absoluto = url::Url::from_file_path(caminho)
                .map(|u| u.to_string())
                .unwrap_or_default();
            if importadas.contains(&uri) || importadas.contains(&absoluto) {
                continue;
            }
            for d in nomes {
                if let Some(q) = crate::aproximado::pontuar(digitado, &d.nome)
                    && !visivel(&d.nome)
                    && (!so_tipos || d.tipo)
                {
                    candidatos.push((q, uri.clone(), d.clone()));
                }
            }
        }
        candidatos.sort_by(|a, b| (a.0, &a.2.nome, &a.1).cmp(&(b.0, &b.2.nome, &b.1)));
        let incompleta = candidatos.len() > TETO_NAO_IMPORTADOS;
        let unit = &programa.unit(unidade).unit;
        for (_, uri, d) in candidatos.into_iter().take(TETO_NAO_IMPORTADOS) {
            let (span, novo) = crate::acoes::inserir_import(texto, unit, &uri);
            let funcao = d.especie == especie::FUNCAO;
            let rotulo = match (funcao, d.sem_parametros) {
                (true, true) => format!("{}()", d.nome),
                (true, false) => format!("{}(…)", d.nome),
                _ => d.nome.clone(),
            };
            let item = self.empurrar(
                grupo::NAO_IMPORTADO,
                d.especie,
                rotulo,
                d.nome.clone(),
                Some(format!("Auto import from '{uri}'")),
            );
            item.importar = Some(ImportAutomatico {
                uri,
                span,
                texto: novo,
            });
            item.origem = Some((d.arquivo.clone(), d.inicio));
            if funcao {
                item.chamada = Some(if d.sem_parametros {
                    Chamada::Parametros(Vec::new())
                } else {
                    Chamada::Desconhecida
                });
            }
        }
        incompleta
    }

    /// Nomes visíveis no identificador sondado.
    fn escopo(&mut self, consulta: &mut Consulta) {
        if let Some(escopo) = consulta.escopo.clone() {
            for local in &escopo.locais {
                let nome = consulta.nome(local.nome).to_string();
                let especie = if local.funcao {
                    especie::FUNCAO
                } else {
                    especie::VARIAVEL
                };
                let detalhe = consulta.formatar(local.tipo);
                self.empurrar(grupo::LOCAL, especie, nome.clone(), nome, Some(detalhe));
            }
            for (nome, _) in &escopo.parametros_de_tipo {
                let nome = consulta.nome(*nome).to_string();
                self.empurrar(
                    grupo::LOCAL,
                    especie::PARAMETRO_DE_TIPO,
                    nome.clone(),
                    nome,
                    None,
                );
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
        let mut membros: Vec<FunctionElementId> = c.static_members.values().copied().collect();
        membros.sort();
        for f in membros {
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
        match consulta.tabela.get(busca).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
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
            let mut nomeados: Vec<(String, TypeId)> = Vec::new();
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
                        nomeados.push((consulta.nome(n).to_string(), p.ty));
                    }
                }
            } else if let Some(t) = alvo.and_then(|a| corpos.get_type(a))
                && let Type::Function { named, .. } = consulta.tabela.get(t)
            {
                for (n, t, _) in named.iter() {
                    if !passados.contains(n) {
                        nomeados.push((consulta.nome(*n).to_string(), *t));
                    }
                }
            }
            for (nome, tipo) in nomeados {
                self.empurrar(
                    grupo::NOMEADO,
                    especie::VARIAVEL,
                    format!("{nome}: "),
                    format!("{nome}: "),
                    Some(consulta.formatar(tipo)),
                );
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
