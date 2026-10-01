//! Identidade semântica das declarações sobre um programa carregado: o que o
//! cursor denota, onde está a declaração e todas as ocorrências dela.
//!
//! É o modelo único de `definition`, `hover`, `references` e `rename`. O que
//! uma referência denota vem das tabelas laterais da inferência comum de
//! `crates/types` (`get_resolved`, `declaracao_local`, `tipos_de_locais`) e
//! dos namespaces de `crates/elements` (`lookup`, `lookup_prefixed`), nunca
//! de uma resolução paralela por nome:
//!
//! * local, parâmetro ou função local — a declaração e as expressões que a
//!   referem; num parâmetro nomeado, os rótulos `nome:` das chamadas e o
//!   parâmetro homônimo das sobrescritas do método;
//! * membro de classe ou extensão — a família ligada por sobrescrita (sobe e
//!   desce pela hierarquia até fechar), getter e setter juntos;
//! * declaração de topo — usos, anotações de tipo, construtores escritos com
//!   o nome da classe, `show`/`hide` e metadados;
//! * construtor nomeado, prefixo de import e parâmetro de tipo;
//! * referências `[nome]` dos comentários de documentação, resolvidas no
//!   escopo da declaração documentada.
//!
//! Um [`Projeto`] é transitório: pertence a uma requisição ou à sessão
//! limitada de [`crate::sessao`], e é descartado ao mudar qualquer texto.

use crate::DocumentStore;
use crate::consulta::Consulta;
use crate::dartdoc;
use crate::semantica::AnalisadorSemantico;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, LibraryId,
    Program, UnitId, VariableId, VariableRef,
};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::{self, Ast, DeclKind, ExprKind, MemberKind, Parameter, StmtKind};
use dartforge_intern::SymbolId;
use dartforge_types::{MemberRef, Resolved};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use url::Url;

/// Teto de arquivos `.dart` varridos na raiz do projeto.
const TETO_ARQUIVOS: usize = 20_000;

/// O programa carregado, com os corpos inferidos das bibliotecas onde se
/// procuram usos.
pub(crate) struct Projeto {
    pub(crate) consulta: Consulta,
    pub(crate) raiz: PathBuf,
    /// Bibliotecas cujos corpos foram inferidos (as do projeto, ou só a do
    /// documento numa consulta de biblioteca). Usos só são procurados nelas.
    pub(crate) bibliotecas: HashSet<LibraryId>,
}

/// O que uma posição denota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Alvo {
    /// Local, parâmetro ou função local: unidade e offset do nome declarado.
    Local { unidade: UnitId, declaracao: usize },
    /// Membro de classe ou extensão, pelo nome base (sem `=` do setter).
    Membro {
        dono: Dono,
        nome: String,
        estatico: bool,
    },
    /// Declaração de topo.
    Topo(Element),
    /// Construtor nomeado (o sem nome denota a classe).
    Construtor(FunctionElementId),
    /// Prefixo de import de uma biblioteca.
    Prefixo {
        biblioteca: LibraryId,
        nome: SymbolId,
    },
    /// Parâmetro de tipo: unidade e offset do nome declarado.
    ParametroDeTipo { unidade: UnitId, declaracao: usize },
}

/// Dono de um membro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dono {
    Classe(ClassId),
    Extensao(ExtensionId),
}

/// O elemento concreto por trás de um [`Alvo`] de membro ou de topo (a
/// família do renomear abrange vários; a definição e o hover querem este).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Concreto {
    Funcao(FunctionElementId),
    Variavel(VariableId),
}

/// O resultado de [`Projeto::identificar`].
#[derive(Debug, Clone)]
pub(crate) struct Denotado {
    pub alvo: Alvo,
    /// O intervalo do nome sob o cursor.
    pub nome: Span,
    /// A expressão da referência, quando o cursor está num uso (o tipo
    /// estático dela é o do hover, já com promoções).
    pub expr: Option<ast::ExprId>,
    /// Função ou variável concreta, quando há.
    pub concreto: Option<Concreto>,
}

/// Raiz do projeto de `arquivo`: o diretório mais próximo com
/// `pubspec.yaml`; sem ele, o próprio diretório do arquivo.
pub(crate) fn raiz_do_projeto(arquivo: &Path) -> PathBuf {
    let mut atual = arquivo.parent();
    while let Some(dir) = atual {
        if dir.join("pubspec.yaml").is_file() {
            return dir.to_path_buf();
        }
        atual = dir.parent();
    }
    arquivo.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// Arquivos `.dart` sob `raiz`, sem diretórios ocultos, `build` nem
/// subpacotes (outro `pubspec.yaml`), em ordem.
pub(crate) fn arquivos_do_projeto(raiz: &Path) -> Vec<PathBuf> {
    let mut saida = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(dir) = pilha.pop() {
        let Ok(entradas) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entradas: Vec<PathBuf> =
            entradas.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entradas.sort();
        for caminho in entradas {
            let nome = caminho.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if caminho.is_dir() {
                if nome.starts_with('.')
                    || nome == "build"
                    || caminho.join("pubspec.yaml").is_file()
                {
                    continue;
                }
                pilha.push(caminho);
            } else if nome.ends_with(".dart") && saida.len() < TETO_ARQUIVOS {
                saida.push(caminho);
            }
        }
    }
    saida.sort();
    saida
}

/// O texto é uma parte (`part of`), que não pode entrar como biblioteca.
pub(crate) fn eh_parte(texto: &str) -> bool {
    if !texto.contains("part") {
        return false;
    }
    let mut nomes = dartforge_intern::Interner::new();
    let analisado = dartforge_frontend::parser::parse(texto, &mut nomes);
    analisado
        .unit
        .directives
        .iter()
        .any(|d| matches!(d.kind, ast::DirectiveKind::PartOf { .. }))
}

/// Caminho canônico (sem `\\?\`) do arquivo de uma URI `file:`.
pub(crate) fn arquivo_da_uri(uri: &str) -> Option<PathBuf> {
    let arquivo = Url::parse(uri).ok()?.to_file_path().ok()?;
    Some(dartforge_elements::config::sem_verbatim(
        std::fs::canonicalize(&arquivo).unwrap_or(arquivo),
    ))
}

/// Carrega o projeto que contém `uri` (todas as bibliotecas sob a raiz, como
/// entradas de uma só carga), com os documentos abertos nos textos vigentes,
/// e infere os corpos das bibliotecas do projeto com o registro de locais.
pub(crate) fn carregar_projeto(
    sdk: &SdkLayout,
    documentos: &DocumentStore,
    uri: &str,
) -> Option<Projeto> {
    let arquivo = arquivo_da_uri(uri)?;
    let raiz = raiz_do_projeto(&arquivo);
    let gerador = AnalisadorSemantico::abertos(documentos);
    let mut entradas: Vec<PathBuf> = Vec::new();
    let texto_de = |caminho: &Path| -> Option<String> {
        let aberto = Url::from_file_path(caminho)
            .ok()
            .and_then(|u| documentos.get(u.as_str()).map(str::to_string));
        aberto.or_else(|| std::fs::read_to_string(caminho).ok())
    };
    let mut candidatos = vec![arquivo.clone()];
    candidatos.extend(
        arquivos_do_projeto(&raiz)
            .into_iter()
            .filter(|c| *c != arquivo),
    );
    // Documentos abertos sob a raiz que ainda não existem no disco.
    for aberto in documentos.uris() {
        if let Some(c) = Url::parse(aberto).ok().and_then(|u| u.to_file_path().ok())
            && c.starts_with(&raiz)
            && c.extension().is_some_and(|e| e == "dart")
            && !candidatos.contains(&c)
        {
            candidatos.push(c);
        }
    }
    for c in candidatos {
        if texto_de(&c).is_some_and(|t| !eh_parte(&t)) {
            entradas.push(c);
        }
    }
    if entradas.is_empty() {
        // Só partes: a biblioteca dona está fora da raiz.
        return None;
    }
    let geracao = gerador.concluir(1).ok()?;
    let mut nomes = dartforge_intern::Interner::new();
    let referencias: Vec<&Path> = entradas.iter().map(PathBuf::as_path).collect();
    let (programa, _) = dartforge_elements::load::load_lenient_entradas(
        &referencias,
        sdk,
        None,
        &mut nomes,
        None,
        None,
        Some(geracao),
    );
    let bibliotecas: HashSet<LibraryId> = programa
        .units
        .iter()
        .filter(|u| {
            !programa.library(u.library).is_sdk
                && u.path.as_deref().is_some_and(|p| dentro(p, &raiz))
        })
        .map(|u| u.library)
        .collect();
    let mut lista: Vec<LibraryId> = bibliotecas.iter().copied().collect();
    lista.sort();
    let consulta = Consulta::inferir(programa, nomes, &lista, true, None);
    Some(Projeto {
        consulta,
        raiz,
        bibliotecas,
    })
}

/// Carrega só a biblioteca de `uri` (e o que ela importa), inferindo os
/// corpos dela: basta para definição e hover, que olham um arquivo.
pub(crate) fn carregar_biblioteca(
    sdk: &SdkLayout,
    documentos: &DocumentStore,
    uri: &str,
) -> Option<Projeto> {
    let texto = documentos.get(uri)?;
    let arquivo = arquivo_da_uri(uri)?;
    let (programa, nomes, unidade) = crate::semantica::carregar(sdk, uri, texto, Some(documentos))?;
    let lib = programa.unit(unidade).library;
    let consulta = Consulta::inferir(programa, nomes, &[lib], true, None);
    Some(Projeto {
        consulta,
        raiz: raiz_do_projeto(&arquivo),
        bibliotecas: HashSet::from([lib]),
    })
}

/// `caminho` está sob `raiz` (comparação pelas chaves canônicas).
pub(crate) fn dentro(caminho: &Path, raiz: &Path) -> bool {
    dartforge_elements::gerado::chave(caminho).starts_with(dartforge_elements::gerado::chave(raiz))
}

/// Byte que pode compor um identificador Dart.
fn eh_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// O identificador que contém `offset` (ou termina nele).
pub(crate) fn palavra(fonte: &str, offset: usize) -> Option<Span> {
    let b = fonte.as_bytes();
    let offset = offset.min(b.len());
    let mut inicio = offset;
    while inicio > 0 && eh_ident(b[inicio - 1]) {
        inicio -= 1;
    }
    let mut fim = offset;
    while fim < b.len() && eh_ident(b[fim]) {
        fim += 1;
    }
    (inicio < fim && !b[inicio].is_ascii_digit()).then_some(Span {
        start: inicio,
        end: fim,
    })
}

/// Nome base de um membro: o setter é guardado como `x=` (ou `x_=`).
pub(crate) fn nome_base(nome: &str) -> &str {
    nome.strip_suffix("_=")
        .or_else(|| nome.strip_suffix('='))
        .unwrap_or(nome)
}

/// `offset` cai dentro de `s` (fim exclusivo).
fn contem(s: Span, offset: usize) -> bool {
    s.start <= offset && offset < s.end
}

impl Projeto {
    /// O programa carregado.
    pub(crate) fn programa(&self) -> &Program {
        &self.consulta.programa
    }

    /// O texto de um símbolo.
    pub(crate) fn nome(&self, s: SymbolId) -> &str {
        self.consulta.nome(s)
    }

    /// Unidades das bibliotecas inferidas, em ordem.
    pub(crate) fn unidades(&self) -> Vec<UnitId> {
        (0..self.programa().units.len())
            .map(|i| UnitId(i as u32))
            .filter(|u| self.bibliotecas.contains(&self.programa().unit(*u).library))
            .collect()
    }

    /// A URI `file:` de uma unidade com caminho.
    pub(crate) fn uri_da_unidade(&self, u: UnitId) -> Option<String> {
        Url::from_file_path(self.programa().unit(u).path.as_ref()?)
            .ok()
            .map(|u| u.to_string())
    }

    /// A unidade carregada do arquivo de `uri` (caminho canônico).
    pub(crate) fn unidade_do_uri(&self, uri: &str) -> Option<UnitId> {
        let caminho = arquivo_da_uri(uri)?;
        let chave = dartforge_elements::gerado::chave(&caminho);
        self.programa()
            .units
            .iter()
            .position(|u| {
                u.path
                    .as_deref()
                    .map(dartforge_elements::gerado::chave)
                    .as_ref()
                    == Some(&chave)
            })
            .map(|i| UnitId(i as u32))
    }

    /// A biblioteca é do projeto (e não do SDK nem de um pacote externo).
    pub(crate) fn do_projeto(&self, lib: LibraryId) -> bool {
        let l = self.programa().library(lib);
        !l.is_sdk
            && l.units.iter().all(|u| {
                self.programa()
                    .unit(*u)
                    .path
                    .as_deref()
                    .is_some_and(|p| dentro(p, &self.raiz))
            })
    }

    /// Biblioteca que declara um elemento de topo.
    pub(crate) fn biblioteca_do_elemento(&self, el: Element) -> LibraryId {
        let p = self.programa();
        match el {
            Element::Class(c) => p.class(c).library,
            Element::Extension(x) => p.extension(x).library,
            Element::Typedef(t) => p.typedef(t).library,
            Element::Function(f) => p.function(f).library,
            Element::Variable(v) => p.variable(v).library,
            Element::Prefix(l, _) => l,
        }
    }

    // -----------------------------------------------------------------
    // O que está sob o cursor
    // -----------------------------------------------------------------

    /// O que a posição denota. `Ok(None)`: não há nome ali (espaço,
    /// palavra-chave, literal). `Err`: há um nome, mas sem resolução.
    pub(crate) fn identificar(
        &self,
        unidade: UnitId,
        offset: usize,
    ) -> Result<Option<Denotado>, String> {
        let u = self.programa().unit(unidade);
        let Some(nome) = palavra(&u.source, offset) else {
            return Ok(None);
        };
        let texto = &u.source[nome.start..nome.end];
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let ast = &u.ast;
        let denotado = |alvo: Alvo, expr: Option<ast::ExprId>, concreto: Option<Concreto>| {
            Ok(Some(Denotado {
                alvo,
                nome,
                expr,
                concreto,
            }))
        };
        // Referências em expressões.
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let referido = match &e.kind {
                ExprKind::Identifier(n) => *n,
                ExprKind::Property { name, .. } => *name,
                ExprKind::InstanceCreation {
                    constructor: Some(n),
                    ..
                } => *n,
                _ => continue,
            };
            if referido.span != nome {
                continue;
            }
            // Prefixo que colide com uma declaração de topo é erro de
            // compilação (`prefix_collides_with_top_level_member`): o que
            // `p` ou `p.x` denota não é afirmado.
            let alvo_prefixado = match &e.kind {
                ExprKind::Property { target, .. } => match &ast.expr(*target).kind {
                    ExprKind::Identifier(p) => Some(p.sym),
                    _ => None,
                },
                ExprKind::Identifier(n) => Some(n.sym),
                _ => None,
            };
            if let Some(p) = alvo_prefixado
                && self.programa().library(u.library).prefixes.contains_key(&p)
                && self.programa().library(u.library).declared.contains_key(&p)
            {
                return Err(format!(
                    "O prefixo '{}' colide com uma declaração de topo.",
                    self.nome(p)
                ));
            }
            // `A.nome(…)` e `A(…)` sem `new`: a resolução do construtor fica
            // na chamada, não no alvo dela.
            let construtor = construtor_da_chamada(ast, corpos, id);
            let resolvido = match (corpos.get_resolved(id), construtor) {
                (None, Some(f)) => Some(Resolved::Constructor(f)),
                (Some(Resolved::Element(Element::Class(c))), Some(f)) => {
                    return denotado(
                        Alvo::Topo(Element::Class(*c)),
                        Some(id),
                        Some(Concreto::Funcao(f)),
                    );
                }
                (r, _) => r.cloned(),
            };
            return match resolvido.as_ref() {
                Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {
                    match corpos.declaracao_local(id) {
                        Some(d) => {
                            let (alvo, concreto) = self.local_ou_campo(unidade, d);
                            denotado(alvo, Some(id), concreto)
                        }
                        None => Ok(None),
                    }
                }
                Some(Resolved::Element(Element::Prefix(_, p))) => denotado(
                    Alvo::Prefixo {
                        biblioteca: u.library,
                        nome: *p,
                    },
                    Some(id),
                    None,
                ),
                Some(Resolved::Prefix(_)) => {
                    let ExprKind::Identifier(n) = &e.kind else {
                        return Ok(None);
                    };
                    denotado(
                        Alvo::Prefixo {
                            biblioteca: u.library,
                            nome: n.sym,
                        },
                        Some(id),
                        None,
                    )
                }
                Some(Resolved::Element(el)) => {
                    let (alvo, concreto) = self.topo(*el);
                    denotado(alvo, Some(id), concreto)
                }
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                })
                | Some(Resolved::ExtensionMember { member: f, .. }) => denotado(
                    self.membro_de_funcao(*f),
                    Some(id),
                    Some(self.concreto_de_funcao(*f)),
                ),
                Some(Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                }) => denotado(
                    self.membro_de_variavel(*v),
                    Some(id),
                    Some(Concreto::Variavel(*v)),
                ),
                Some(Resolved::Constructor(f)) => {
                    let fe = self.programa().function(*f);
                    match fe.class {
                        Some(c) if self.nome(self.programa().class(c).name) == texto => denotado(
                            Alvo::Topo(Element::Class(c)),
                            Some(id),
                            Some(Concreto::Funcao(*f)),
                        ),
                        _ => denotado(Alvo::Construtor(*f), Some(id), Some(Concreto::Funcao(*f))),
                    }
                }
                Some(Resolved::TypeParameter(_)) => {
                    match declaracao_de_parametro_de_tipo(ast, nome.start, referido.sym) {
                        Some(d) => denotado(
                            Alvo::ParametroDeTipo {
                                unidade,
                                declaracao: d,
                            },
                            Some(id),
                            None,
                        ),
                        None => Ok(None),
                    }
                }
                _ => Err(format!("'{texto}' não está resolvido.")),
            };
        }
        // Declaração de local ou parâmetro.
        if corpos.tipo_local(nome.start).is_some() {
            let (alvo, concreto) = self.local_ou_campo(unidade, nome.start);
            return denotado(alvo, None, concreto);
        }
        // Anotações de tipo.
        for t in &ast.types {
            let ast::TypeKind::Named { name, .. } = &t.kind else {
                continue;
            };
            let Some(pos) = name.iter().position(|n| n.span == nome) else {
                continue;
            };
            let lib = u.library;
            // `A.nome` como tipo é a forma de `new A.nome()` e de `= A.nome`:
            // a classe e o construtor nomeado, não prefixo e tipo.
            if let [classe, construtor] = &name[..]
                && !eh_prefixo(self.programa(), lib, classe.sym)
                && let Some(Element::Class(c)) = self
                    .programa()
                    .lookup(lib, classe.sym)
                    .and_then(|b| b.getter)
            {
                if pos == 0 {
                    let (alvo, concreto) = self.topo(Element::Class(c));
                    return denotado(alvo, None, concreto);
                }
                return match self.programa().class(c).constructors.get(&construtor.sym) {
                    Some(f) => denotado(Alvo::Construtor(*f), None, Some(Concreto::Funcao(*f))),
                    None => Err(format!("O construtor '{texto}' não está resolvido.")),
                };
            }
            if pos + 1 < name.len() {
                // `p` em `p.Nome`.
                return denotado(
                    Alvo::Prefixo {
                        biblioteca: lib,
                        nome: name[pos].sym,
                    },
                    None,
                    None,
                );
            }
            if name.len() == 1
                && let Some(d) = declaracao_de_parametro_de_tipo(ast, nome.start, name[0].sym)
            {
                return denotado(
                    Alvo::ParametroDeTipo {
                        unidade,
                        declaracao: d,
                    },
                    None,
                    None,
                );
            }
            let vinculo = match &name[..] {
                [n] => self.programa().lookup(lib, n.sym),
                [p, n] => self.programa().lookup_prefixed(lib, p.sym, n.sym),
                _ => None,
            };
            return match vinculo.and_then(|b| b.getter) {
                Some(el) => {
                    let (alvo, concreto) = self.topo(el);
                    denotado(alvo, None, concreto)
                }
                None => Err(format!("O tipo '{texto}' não está resolvido.")),
            };
        }
        // Prefixo na diretiva `import … as p`.
        for d in &u.unit.directives {
            if let ast::DirectiveKind::Import {
                prefix: Some(p), ..
            } = &d.kind
                && p.span == nome
            {
                return denotado(
                    Alvo::Prefixo {
                        biblioteca: u.library,
                        nome: p.sym,
                    },
                    None,
                    None,
                );
            }
        }
        // Nomes de `show`/`hide` de um import ou export: o elemento que a
        // biblioteca alvo exporta com esse nome.
        {
            let p = self.programa();
            let lib = p.library(u.library);
            let alvos = lib
                .imports
                .iter()
                .filter(|i| i.unit == unidade)
                .map(|i| (i.directive, i.library))
                .chain(lib.exports.iter().filter(|e| e.unit == unidade).map(|e| (e.directive, e.library)));
            for (indice, alvo_lib) in alvos {
                let Some(d) = u.unit.directives.get(indice) else { continue };
                let combinadores = match &d.kind {
                    ast::DirectiveKind::Import { combinators, .. } | ast::DirectiveKind::Export { combinators, .. } => combinators,
                    _ => continue,
                };
                for c in combinadores {
                    let (ast::Combinator::Show(nomes) | ast::Combinator::Hide(nomes)) = c;
                    if let Some(n) = nomes.iter().find(|n| n.span == nome)
                        && let Some(el) = p.library(alvo_lib).exported.get(&n.sym).and_then(|b| b.getter.or(b.setter))
                    {
                        let (alvo, concreto) = self.topo(el);
                        return denotado(alvo, None, concreto);
                    }
                }
            }
        }
        // Metadados (`@nome`, `@p.nome`, `@Classe.ctor(...)`).
        if let Some(d) = self.em_metadados(unidade, nome) {
            return Ok(Some(d));
        }
        // Referências de comentários de documentação.
        if let Some(d) = self.em_dartdoc(unidade, nome) {
            return Ok(Some(d));
        }
        // Declarações.
        if let Some((alvo, concreto)) = self.declaracao_em(unidade, nome) {
            return denotado(alvo, None, concreto);
        }
        Ok(None)
    }

    /// Local no offset `declaracao`; parâmetro `this.x` vira o campo `x`.
    fn local_ou_campo(&self, unidade: UnitId, declaracao: usize) -> (Alvo, Option<Concreto>) {
        let ast = &self.programa().unit(unidade).ast;
        if let Some((p, _)) = parametro_em(ast, declaracao)
            && p.this_
            && let Some(r) = self.campo_do_parametro(unidade, declaracao)
        {
            return r;
        }
        (
            Alvo::Local {
                unidade,
                declaracao,
            },
            None,
        )
    }

    /// O campo inicializado pelo parâmetro `this.x` declarado em `offset`.
    fn campo_do_parametro(
        &self,
        unidade: UnitId,
        offset: usize,
    ) -> Option<(Alvo, Option<Concreto>)> {
        let ast = &self.programa().unit(unidade).ast;
        let (p, _) = parametro_em(ast, offset)?;
        let simbolo = p.name?.sym;
        let classe = self.programa().classes.iter().position(|c| {
            c.decl.is_some_and(|d| {
                d.unit == unidade
                    && contem(self.programa().unit(d.unit).ast.decl(d.decl).span, offset)
            })
        })?;
        let classe = ClassId(classe as u32);
        let campo = self
            .programa()
            .class(classe)
            .fields
            .iter()
            .copied()
            .find(|v| self.programa().variable(*v).name == simbolo);
        Some((
            Alvo::Membro {
                dono: Dono::Classe(classe),
                nome: self.nome(simbolo).to_string(),
                estatico: false,
            },
            campo.map(Concreto::Variavel),
        ))
    }

    /// Alvo e concreto de um elemento de topo (acessor implícito vira a variável).
    fn topo(&self, el: Element) -> (Alvo, Option<Concreto>) {
        match el {
            Element::Function(f) => {
                let fe = self.programa().function(f);
                if fe.kind == FunctionKind::ImplicitAccessor
                    && let Some(v) = fe.variable
                {
                    return (
                        Alvo::Topo(Element::Variable(v)),
                        Some(Concreto::Variavel(v)),
                    );
                }
                (Alvo::Topo(el), Some(Concreto::Funcao(f)))
            }
            Element::Variable(v) => (Alvo::Topo(el), Some(Concreto::Variavel(v))),
            Element::Prefix(l, p) => (
                Alvo::Prefixo {
                    biblioteca: l,
                    nome: p,
                },
                None,
            ),
            _ => (Alvo::Topo(el), None),
        }
    }

    /// O alvo de um membro função (acessor implícito vira a variável).
    pub(crate) fn membro_de_funcao(&self, f: FunctionElementId) -> Alvo {
        let fe = self.programa().function(f);
        if let Some(v) = fe.variable
            && fe.kind == FunctionKind::ImplicitAccessor
        {
            return self.membro_de_variavel(v);
        }
        let nome = nome_base(self.nome(fe.name)).to_string();
        let dono = match (fe.class, fe.extension) {
            (_, Some(x)) => Dono::Extensao(x),
            (Some(c), _) => Dono::Classe(c),
            (None, None) => return Alvo::Topo(Element::Function(f)),
        };
        Alvo::Membro {
            dono,
            nome,
            estatico: fe.static_,
        }
    }

    /// O alvo de um campo ou constante de enum (de topo vira [`Alvo::Topo`]).
    pub(crate) fn membro_de_variavel(&self, v: VariableId) -> Alvo {
        let ve = self.programa().variable(v);
        let nome = self.nome(ve.name).to_string();
        let dono = match (ve.class, ve.extension) {
            (_, Some(x)) => Dono::Extensao(x),
            (Some(c), _) => Dono::Classe(c),
            (None, None) => return Alvo::Topo(Element::Variable(v)),
        };
        Alvo::Membro {
            dono,
            nome,
            estatico: ve.static_,
        }
    }

    /// Declaração (de topo, membro, construtor, constante de enum, parâmetro
    /// de tipo) cujo nome é `nome`.
    fn declaracao_em(&self, unidade: UnitId, nome: Span) -> Option<(Alvo, Option<Concreto>)> {
        let programa = self.programa();
        let u = programa.unit(unidade);
        let ast = &u.ast;
        for &d in &u.unit.declarations {
            let decl = ast.decl(d);
            let nome_decl = match &decl.kind {
                DeclKind::Class(c) => Some(c.name),
                DeclKind::Mixin(m) => Some(m.name),
                DeclKind::Enum(e) => Some(e.name),
                DeclKind::ExtensionType(e) => Some(e.name),
                DeclKind::Typedef(t) => Some(t.name),
                DeclKind::Extension(x) => x.name,
                DeclKind::Function(f) => ast.function(*f).name,
                DeclKind::Variables(vl) => {
                    vl.variables.iter().map(|v| v.name).find(|n| n.span == nome)
                }
            };
            if let Some(n) = nome_decl.filter(|n| n.span == nome) {
                let el = match &decl.kind {
                    DeclKind::Function(fid) => {
                        self.funcao_do_no(unidade, *fid).map(Element::Function)
                    }
                    DeclKind::Variables(vl) => {
                        let i = vl.variables.iter().position(|v| v.name.span == nome)?;
                        self.variavel_do_no(VariableRef::TopLevel {
                            unit: unidade,
                            decl: d,
                            index: i,
                        })
                        .map(Element::Variable)
                    }
                    _ => programa
                        .library(u.library)
                        .declared
                        .get(&n.sym)
                        .and_then(|b| b.getter.or(b.setter)),
                };
                return el.map(|el| self.topo(el));
            }
            if let DeclKind::Enum(e) = &decl.kind {
                for (i, c) in e.constants.iter().enumerate() {
                    if c.name.span == nome
                        && let Some(v) = self.variavel_do_no(VariableRef::EnumConstant {
                            unit: unidade,
                            decl: d,
                            index: i,
                        })
                    {
                        return Some((self.membro_de_variavel(v), Some(Concreto::Variavel(v))));
                    }
                }
            }
        }
        for (mi, m) in ast.members.iter().enumerate() {
            let mid = ast::MemberId(mi as u32);
            match &m.kind {
                MemberKind::Method(fid) => {
                    if ast.function(*fid).name.is_some_and(|n| n.span == nome)
                        && let Some(f) = self.funcao_do_no(unidade, *fid)
                    {
                        return Some((self.membro_de_funcao(f), Some(Concreto::Funcao(f))));
                    }
                }
                MemberKind::Field(vl) => {
                    for (i, v) in vl.variables.iter().enumerate() {
                        if v.name.span == nome
                            && let Some(vid) = self.variavel_do_no(VariableRef::Field {
                                unit: unidade,
                                member: mid,
                                index: i,
                            })
                        {
                            return Some((
                                self.membro_de_variavel(vid),
                                Some(Concreto::Variavel(vid)),
                            ));
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    let f = self.construtor_do_no(unidade, mid);
                    if k.class_name.span == nome {
                        let classe = programa
                            .library(u.library)
                            .declared
                            .get(&k.class_name.sym)
                            .and_then(|b| b.getter);
                        return match classe {
                            Some(el @ Element::Class(_)) => {
                                Some((Alvo::Topo(el), f.map(Concreto::Funcao)))
                            }
                            _ => None,
                        };
                    }
                    if k.name.is_some_and(|n| n.span == nome) {
                        let f = f?;
                        return Some((Alvo::Construtor(f), Some(Concreto::Funcao(f))));
                    }
                }
            }
        }
        // Parâmetros de tipo declarados.
        let declara = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.span == nome);
        let em_parametros_de_tipo = ast.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(c) => declara(&c.type_params),
            DeclKind::Mixin(m) => declara(&m.type_params),
            DeclKind::Enum(e) => declara(&e.type_params),
            DeclKind::Extension(x) => declara(&x.type_params),
            DeclKind::ExtensionType(x) => declara(&x.type_params),
            DeclKind::Typedef(t) => declara(&t.type_params),
            _ => false,
        }) || ast.functions.iter().any(|f| declara(&f.type_params))
            || ast.types.iter().any(|t| {
                matches!(&t.kind, ast::TypeKind::Function { type_params, .. } if declara(type_params))
            });
        if em_parametros_de_tipo {
            return Some((
                Alvo::ParametroDeTipo {
                    unidade,
                    declaracao: nome.start,
                },
                None,
            ));
        }
        None
    }

    /// Nome sob o cursor numa anotação de metadados.
    fn em_metadados(&self, unidade: UnitId, nome: Span) -> Option<Denotado> {
        let u = self.programa().unit(unidade);
        for a in metadados(&u.ast, &u.unit) {
            let Some(pos) = a.name.iter().position(|n| n.span == nome) else {
                continue;
            };
            let partes: Vec<SymbolId> = a.name.iter().map(|n| n.sym).collect();
            let (alvo, concreto) = self.resolver_caminho(u.library, None, &partes[..=pos])?;
            return Some(Denotado {
                alvo,
                nome,
                expr: None,
                concreto,
            });
        }
        None
    }

    /// Nome sob o cursor numa referência `[…]` de documentação.
    fn em_dartdoc(&self, unidade: UnitId, nome: Span) -> Option<Denotado> {
        let u = self.programa().unit(unidade);
        for c in dartdoc::comentarios(&u.source) {
            if !contem(c.span, nome.start) {
                continue;
            }
            for r in &c.referencias {
                let Some(pos) = r.iter().position(|(s, _)| *s == nome) else {
                    continue;
                };
                let (alvo, concreto) = self.resolver_referencia_doc(unidade, c.span, &r[..=pos])?;
                return Some(Denotado {
                    alvo,
                    nome,
                    expr: None,
                    concreto,
                });
            }
        }
        None
    }

    /// Resolve `a`, `a.b`, `a.b.c` a partir do escopo da biblioteca `lib`
    /// (metadados e documentação): prefixo, elemento de topo, membro
    /// estático ou de instância e construtor nomeado. Com `classe`, um nome
    /// simples procura antes nos membros dela (documentação de membro).
    fn resolver_caminho(
        &self,
        lib: LibraryId,
        classe: Option<ClassId>,
        partes: &[SymbolId],
    ) -> Option<(Alvo, Option<Concreto>)> {
        let p = self.programa();
        let (primeiro, resto) = partes.split_first()?;
        if resto.is_empty()
            && let Some(c) = classe
            && let Some(r) = self.membro_por_nome(c, *primeiro)
        {
            return Some(r);
        }
        let mut atual: Option<Element> = None;
        let mut resto = resto;
        if eh_prefixo(p, lib, *primeiro) {
            if resto.is_empty() {
                return Some((
                    Alvo::Prefixo {
                        biblioteca: lib,
                        nome: *primeiro,
                    },
                    None,
                ));
            }
            atual = p.lookup_prefixed(lib, *primeiro, resto[0])?.getter;
            resto = &resto[1..];
        } else {
            atual = atual.or(p.lookup(lib, *primeiro).and_then(|b| b.getter.or(b.setter)));
        }
        let el = atual?;
        match resto {
            [] => Some(self.topo(el)),
            [membro] => match el {
                Element::Class(c) => {
                    if let Some(f) = p.class(c).constructors.get(membro).copied() {
                        return Some((Alvo::Construtor(f), Some(Concreto::Funcao(f))));
                    }
                    self.membro_por_nome(c, *membro)
                }
                Element::Extension(x) => {
                    let ext = p.extension(x);
                    let f = ext
                        .static_members
                        .get(membro)
                        .or_else(|| ext.instance_members.get(membro))
                        .copied()?;
                    Some((self.membro_de_funcao(f), Some(Concreto::Funcao(f))))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Membro `nome` de `c` (estático, de instância ou herdado), com o concreto.
    fn membro_por_nome(&self, c: ClassId, nome: SymbolId) -> Option<(Alvo, Option<Concreto>)> {
        let p = self.programa();
        let mut classes = vec![c];
        let mut vistas = HashSet::new();
        while let Some(x) = classes.pop() {
            if !vistas.insert(x) {
                continue;
            }
            let cl = p.class(x);
            if let Some(f) = cl
                .static_members
                .get(&nome)
                .or_else(|| cl.instance_members.get(&nome))
                .copied()
            {
                return Some((self.membro_de_funcao(f), Some(self.concreto_de_funcao(f))));
            }
            if x == c
                && let Some(v) = cl
                    .fields
                    .iter()
                    .chain(&cl.enum_constants)
                    .copied()
                    .find(|v| p.variable(*v).name == nome)
            {
                return Some((self.membro_de_variavel(v), Some(Concreto::Variavel(v))));
            }
            classes.extend(cl.supertype_class);
            classes.extend(cl.mixin_classes.iter().copied());
            classes.extend(cl.interface_classes.iter().copied());
        }
        None
    }

    /// Acessor implícito vira a variável; o resto fica a função.
    pub(crate) fn concreto_de_funcao(&self, f: FunctionElementId) -> Concreto {
        let fe = self.programa().function(f);
        match fe.variable {
            Some(v) if fe.kind == FunctionKind::ImplicitAccessor => Concreto::Variavel(v),
            _ => Concreto::Funcao(f),
        }
    }

    /// A declaração documentada pelo comentário `comentario`: a primeira
    /// declaração, membro ou constante de enum que começa depois dele.
    fn documentado(&self, unidade: UnitId, comentario: Span) -> Option<Documentado> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let mut melhor: Option<(usize, Documentado)> = None;
        let mut considerar = |inicio: usize, d: Documentado| {
            if inicio >= comentario.end && melhor.as_ref().is_none_or(|(i, _)| inicio < *i) {
                melhor = Some((inicio, d));
            }
        };
        for (i, d) in ast.decls.iter().enumerate() {
            considerar(d.span.start, Documentado::Decl(ast::DeclId(i as u32)));
        }
        for (i, m) in ast.members.iter().enumerate() {
            considerar(m.span.start, Documentado::Membro(ast::MemberId(i as u32)));
        }
        melhor.map(|(_, d)| d)
    }

    /// Resolve uma referência de documentação (`[a]`, `[A.b]`, `[p.A.b]`)
    /// no escopo da declaração documentada: os parâmetros e parâmetros de
    /// tipo dela, os membros da classe envolvente e o escopo da biblioteca.
    pub(crate) fn resolver_referencia_doc(
        &self,
        unidade: UnitId,
        comentario: Span,
        partes: &[(Span, String)],
    ) -> Option<(Alvo, Option<Concreto>)> {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let simbolos: Vec<SymbolId> = partes
            .iter()
            .map(|(s, _)| {
                // O nome já foi internado se aparece no programa; senão não resolve.
                self.consulta.nomes.lookup(&u.source[s.start..s.end])
            })
            .collect::<Option<Vec<_>>>()?;
        let documentado = self.documentado(unidade, comentario);
        // Um nome simples: parâmetros e parâmetros de tipo da declaração.
        if let [simbolo] = simbolos[..] {
            let (funcao, tipos): (Option<&[Parameter]>, Vec<&ast::TypeParameter>) =
                match documentado {
                    Some(Documentado::Decl(d)) => match &ast.decl(d).kind {
                        DeclKind::Function(f) => (
                            ast.function(*f).parameters.as_deref(),
                            ast.function(*f).type_params.iter().collect(),
                        ),
                        DeclKind::Class(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Mixin(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Enum(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Extension(c) => (None, c.type_params.iter().collect()),
                        DeclKind::ExtensionType(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Typedef(c) => (None, c.type_params.iter().collect()),
                        DeclKind::Variables(_) => (None, Vec::new()),
                    },
                    Some(Documentado::Membro(m)) => match &ast.member(m).kind {
                        MemberKind::Method(f) => (
                            ast.function(*f).parameters.as_deref(),
                            ast.function(*f).type_params.iter().collect(),
                        ),
                        MemberKind::Constructor(k) => (Some(&k.parameters[..]), Vec::new()),
                        MemberKind::Field(_) => (None, Vec::new()),
                    },
                    None => (None, Vec::new()),
                };
            if let Some(p) = funcao
                .into_iter()
                .flatten()
                .find(|p| p.name.is_some_and(|n| n.sym == simbolo))
                && let Some(n) = p.name
            {
                return Some(self.local_ou_campo(unidade, n.span.start));
            }
            if let Some(t) = tipos.iter().find(|t| t.name.sym == simbolo) {
                return Some((
                    Alvo::ParametroDeTipo {
                        unidade,
                        declaracao: t.name.span.start,
                    },
                    None,
                ));
            }
        }
        // Classe envolvente (ou a própria classe documentada).
        let offset = match documentado {
            Some(Documentado::Decl(d)) => ast.decl(d).span.start,
            Some(Documentado::Membro(m)) => ast.member(m).span.start,
            None => comentario.end,
        };
        let classe = self
            .programa()
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.decl.is_some_and(|d| {
                    d.unit == unidade && {
                        let s = ast.decl(d.decl).span;
                        s.start <= offset && offset < s.end
                    }
                })
            })
            .map(|(i, _)| ClassId(i as u32))
            .next();
        self.resolver_caminho(u.library, classe, &simbolos)
    }

    /// Elemento de função declarado pelo nó `fid` da unidade.
    pub(crate) fn funcao_do_no(
        &self,
        unidade: UnitId,
        fid: ast::FunctionId,
    ) -> Option<FunctionElementId> {
        self.programa()
            .functions
            .iter()
            .position(|f| {
                f.node
                    == FunctionRef::Function {
                        unit: unidade,
                        function: fid,
                    }
            })
            .map(|i| FunctionElementId(i as u32))
    }

    /// Elemento do construtor declarado pelo membro `mid`.
    pub(crate) fn construtor_do_no(
        &self,
        unidade: UnitId,
        mid: ast::MemberId,
    ) -> Option<FunctionElementId> {
        self.programa()
            .functions
            .iter()
            .position(|f| {
                f.node
                    == FunctionRef::Constructor {
                        unit: unidade,
                        member: mid,
                    }
            })
            .map(|i| FunctionElementId(i as u32))
    }

    /// O elemento da variável declarada no nó `no`.
    fn variavel_do_no(&self, no: VariableRef) -> Option<VariableId> {
        self.programa()
            .variables
            .iter()
            .position(|v| v.node == no)
            .map(|i| VariableId(i as u32))
    }

    // -----------------------------------------------------------------
    // Declarações
    // -----------------------------------------------------------------

    /// Unidade e span do nome de uma variável de topo, campo ou constante.
    pub(crate) fn nome_da_variavel(&self, v: VariableId) -> Option<(UnitId, Span)> {
        let p = self.programa();
        match p.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => {
                match &p.unit(unit).ast.decl(decl).kind {
                    DeclKind::Variables(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &p.unit(unit).ast.member(member).kind {
                MemberKind::Field(vl) => Some((unit, vl.variables.get(index)?.name.span)),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => {
                match &p.unit(unit).ast.decl(decl).kind {
                    DeclKind::Enum(e) => Some((unit, e.constants.get(index)?.name.span)),
                    _ => None,
                }
            }
            VariableRef::Representation { unit, decl } => match &p.unit(unit).ast.decl(decl).kind {
                DeclKind::ExtensionType(x) => Some((unit, x.representation_name.span)),
                _ => None,
            },
            VariableRef::None => None,
        }
    }

    /// Unidade e span do nome de uma função (topo, método, construtor).
    pub(crate) fn nome_da_funcao(&self, f: FunctionElementId) -> Option<(UnitId, Span)> {
        let p = self.programa();
        let fe = p.function(f);
        match fe.node {
            FunctionRef::Function { unit, function } => {
                Some((unit, p.unit(unit).ast.function(function).name?.span))
            }
            FunctionRef::Constructor { unit, member } => {
                match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => Some((unit, k.name.unwrap_or(k.class_name).span)),
                    _ => None,
                }
            }
            FunctionRef::None => match (fe.variable, fe.class) {
                (Some(v), _) => self.nome_da_variavel(v),
                // Construtor sintético: a classe.
                (None, Some(c)) => self.nome_do_elemento_de_topo(Element::Class(c)),
                _ => None,
            },
        }
    }

    /// Unidade e span do nome de um elemento de topo.
    pub(crate) fn nome_do_elemento_de_topo(&self, el: Element) -> Option<(UnitId, Span)> {
        let p = self.programa();
        match el {
            Element::Class(c) => {
                let d = p.class(c).decl?;
                let n = match &p.unit(d.unit).ast.decl(d.decl).kind {
                    DeclKind::Class(x) => x.name,
                    DeclKind::Mixin(x) => x.name,
                    DeclKind::Enum(x) => x.name,
                    DeclKind::ExtensionType(x) => x.name,
                    _ => return None,
                };
                Some((d.unit, n.span))
            }
            Element::Typedef(t) => {
                let d = p.typedef(t).decl;
                match &p.unit(d.unit).ast.decl(d.decl).kind {
                    DeclKind::Typedef(x) => Some((d.unit, x.name.span)),
                    _ => None,
                }
            }
            Element::Extension(x) => {
                let d = p.extension(x).decl;
                match &p.unit(d.unit).ast.decl(d.decl).kind {
                    DeclKind::Extension(xd) => Some((d.unit, xd.name?.span)),
                    _ => None,
                }
            }
            Element::Function(f) => self.nome_da_funcao(f),
            Element::Variable(v) => self.nome_da_variavel(v),
            Element::Prefix(l, s) => self.declaracao_do_prefixo(l, s),
        }
    }

    /// O `as p` do primeiro import da biblioteca com esse prefixo.
    pub(crate) fn declaracao_do_prefixo(
        &self,
        lib: LibraryId,
        nome: SymbolId,
    ) -> Option<(UnitId, Span)> {
        let p = self.programa();
        p.library(lib).imports.iter().find_map(|i| {
            if i.prefix != Some(nome) {
                return None;
            }
            match &p.unit(i.unit).unit.directives.get(i.directive)?.kind {
                ast::DirectiveKind::Import {
                    prefix: Some(n), ..
                } => Some((i.unit, n.span)),
                _ => None,
            }
        })
    }

    /// Onde o denotado é declarado: o elemento concreto quando há (o membro
    /// resolvido, não a família), senão o alvo.
    pub(crate) fn declaracao(&self, d: &Denotado) -> Option<(UnitId, Span)> {
        match (&d.alvo, d.concreto) {
            (Alvo::Topo(Element::Class(c)), Some(Concreto::Funcao(f)))
                if self.programa().function(f).kind == FunctionKind::Constructor =>
            {
                // `A()` com construtor escrito vai para ele.
                self.nome_da_funcao(f)
                    .or_else(|| self.nome_do_elemento_de_topo(Element::Class(*c)))
            }
            (_, Some(Concreto::Funcao(f))) => self.nome_da_funcao(f),
            (_, Some(Concreto::Variavel(v))) => self.nome_da_variavel(v),
            (alvo, None) => self.declaracao_do_alvo(alvo),
        }
    }

    /// Onde o alvo é declarado (a primeira declaração, na família).
    pub(crate) fn declaracao_do_alvo(&self, alvo: &Alvo) -> Option<(UnitId, Span)> {
        match alvo {
            Alvo::Local {
                unidade,
                declaracao,
            }
            | Alvo::ParametroDeTipo {
                unidade,
                declaracao,
            } => {
                let fonte = &self.programa().unit(*unidade).source;
                Some((*unidade, palavra(fonte, *declaracao)?))
            }
            Alvo::Topo(el) => self.nome_do_elemento_de_topo(*el),
            Alvo::Construtor(f) => self.nome_da_funcao(*f),
            Alvo::Prefixo { biblioteca, nome } => self.declaracao_do_prefixo(*biblioteca, *nome),
            Alvo::Membro {
                dono,
                nome,
                estatico,
            } => {
                let p = self.programa();
                let (funcoes, variaveis): (Vec<FunctionElementId>, Vec<VariableId>) = match dono {
                    Dono::Classe(c) => {
                        let cl = p.class(*c);
                        (
                            self.declarados(*c, nome, *estatico),
                            cl.fields
                                .iter()
                                .chain(&cl.enum_constants)
                                .copied()
                                .filter(|v| self.nome(p.variable(*v).name) == nome)
                                .collect(),
                        )
                    }
                    Dono::Extensao(x) => {
                        let ext = p.extension(*x);
                        let mapa = if *estatico {
                            &ext.static_members
                        } else {
                            &ext.instance_members
                        };
                        let mut f: Vec<FunctionElementId> = mapa
                            .iter()
                            .filter(|(s, _)| nome_base(self.nome(**s)) == nome)
                            .map(|(_, f)| *f)
                            .collect();
                        f.sort();
                        (f, Vec::new())
                    }
                };
                variaveis
                    .into_iter()
                    .find_map(|v| self.nome_da_variavel(v))
                    .or_else(|| funcoes.into_iter().find_map(|f| self.nome_da_funcao(f)))
            }
        }
    }

    /// Todos os nomes que declaram o alvo (a família inteira num membro;
    /// getter e setter homônimos; o `as p` de cada import de um prefixo).
    pub(crate) fn declaracoes(&self, alvo: &Alvo) -> BTreeSet<(UnitId, usize, usize)> {
        let mut saida = BTreeSet::new();
        let mut por = |u: UnitId, s: Span| {
            saida.insert((u, s.start, s.end));
        };
        match alvo {
            Alvo::Membro {
                dono,
                nome,
                estatico,
            } => {
                if let Ok(fam) = self.familia(*dono, nome, *estatico, false) {
                    for f in &fam.funcoes {
                        if let Some((u, s)) = self.nome_da_funcao(*f) {
                            por(u, s);
                        }
                    }
                    for v in &fam.variaveis {
                        if let Some((u, s)) = self.nome_da_variavel(*v) {
                            por(u, s);
                        }
                    }
                }
            }
            Alvo::Topo(el) => {
                for e in self.elementos_de_topo(*el) {
                    if let Some((u, s)) = self.nome_do_elemento_de_topo(e) {
                        por(u, s);
                    }
                }
            }
            Alvo::Prefixo { biblioteca, nome } => {
                let p = self.programa();
                for i in &p.library(*biblioteca).imports {
                    if i.prefix == Some(*nome)
                        && let Some(d) = p.unit(i.unit).unit.directives.get(i.directive)
                        && let ast::DirectiveKind::Import {
                            prefix: Some(n), ..
                        } = &d.kind
                    {
                        por(i.unit, n.span);
                    }
                }
            }
            _ => {
                if let Some((u, s)) = self.declaracao_do_alvo(alvo) {
                    por(u, s);
                }
            }
        }
        saida
    }

    // -----------------------------------------------------------------
    // Ocorrências
    // -----------------------------------------------------------------

    /// Todas as ocorrências do alvo nas bibliotecas inferidas (declarações
    /// incluídas), como `(unidade, início, fim)`, ordenadas e sem repetição.
    ///
    /// `recusar_externos`: membro cuja família inclui declaração fora do
    /// projeto é erro (renomear); as referências aceitam.
    ///
    /// # Erros
    ///
    /// Com `recusar_externos`, a mensagem que explica por que a família do
    /// membro não pode mudar.
    pub(crate) fn ocorrencias(
        &self,
        alvo: &Alvo,
        recusar_externos: bool,
    ) -> Result<BTreeSet<(UnitId, usize, usize)>, String> {
        let mut saida = BTreeSet::new();
        let mut por = |u: UnitId, s: Span| {
            saida.insert((u, s.start, s.end));
        };
        match alvo {
            Alvo::Local {
                unidade,
                declaracao,
            } => {
                for (u, s) in self.ocorrencias_de_local(*unidade, *declaracao, recusar_externos)? {
                    por(u, s);
                }
            }
            Alvo::Topo(el) => self.ocorrencias_de_topo(*el, &mut por),
            Alvo::Membro {
                dono,
                nome,
                estatico,
            } => self.ocorrencias_de_membro(*dono, nome, *estatico, recusar_externos, &mut por)?,
            Alvo::Construtor(f) => self.ocorrencias_de_construtor(*f, &mut por),
            Alvo::Prefixo { biblioteca, nome } => {
                self.ocorrencias_de_prefixo(*biblioteca, *nome, &mut por)
            }
            Alvo::ParametroDeTipo {
                unidade,
                declaracao,
            } => self.ocorrencias_de_parametro_de_tipo(*unidade, *declaracao, &mut por),
        }
        // Referências de documentação.
        for u in self.unidades() {
            let fonte = &self.programa().unit(u).source;
            if !fonte.contains('[') {
                continue;
            }
            for c in dartdoc::comentarios(fonte) {
                for r in &c.referencias {
                    for i in 0..r.len() {
                        if let Some((a, _)) = self.resolver_referencia_doc(u, c.span, &r[..=i])
                            && self.mesmo_alvo(&a, alvo)
                        {
                            por(u, r[i].0);
                        }
                    }
                }
            }
        }
        Ok(saida)
    }

    /// Dois alvos denotam a mesma coisa (um membro, pela família).
    fn mesmo_alvo(&self, a: &Alvo, b: &Alvo) -> bool {
        match (a, b) {
            (
                Alvo::Membro {
                    dono: d1,
                    nome: n1,
                    estatico: e1,
                },
                Alvo::Membro {
                    dono: d2,
                    nome: n2,
                    estatico: e2,
                },
            ) => {
                if n1 != n2 || e1 != e2 {
                    return false;
                }
                if d1 == d2 {
                    return true;
                }
                match (
                    self.familia(*d1, n1, *e1, false),
                    self.familia(*d2, n2, *e2, false),
                ) {
                    (Ok(f1), Ok(f2)) => {
                        !f1.funcoes.is_disjoint(&f2.funcoes)
                            || !f1.variaveis.is_disjoint(&f2.variaveis)
                    }
                    _ => false,
                }
            }
            (Alvo::Topo(x), Alvo::Topo(y)) => x == y || self.elementos_de_topo(*x).contains(y),
            _ => a == b,
        }
    }

    /// Local, parâmetro ou função local: a declaração, as expressões que a
    /// referem e, num parâmetro nomeado, os rótulos das chamadas e o
    /// parâmetro homônimo das sobrescritas (com os usos e rótulos deles).
    fn ocorrencias_de_local(
        &self,
        unidade: UnitId,
        declaracao: usize,
        recusar_externos: bool,
    ) -> Result<Vec<(UnitId, Span)>, String> {
        let mut saida = Vec::new();
        let u = self.programa().unit(unidade);
        let usos_de = |unidade: UnitId, declaracao: usize, saida: &mut Vec<(UnitId, Span)>| {
            let u = self.programa().unit(unidade);
            let fim = palavra(&u.source, declaracao).map_or(declaracao, |s| s.end);
            saida.push((
                unidade,
                Span {
                    start: declaracao,
                    end: fim,
                },
            ));
            let corpos = &self.consulta.corpos.units[unidade.0 as usize];
            for (e, d) in &corpos.declaracoes_de_locais {
                if *d == declaracao
                    && let ExprKind::Identifier(n) = &u.ast.expr(*e).kind
                {
                    saida.push((unidade, n.span));
                }
            }
        };
        usos_de(unidade, declaracao, &mut saida);
        let Some((p, dono)) = parametro_em(&u.ast, declaracao) else {
            return Ok(saida);
        };
        if p.kind != ast::ParameterKind::Named || p.public_name.is_some() {
            return Ok(saida);
        }
        let Some(rotulo) = p.name.map(|n| n.sym) else {
            return Ok(saida);
        };
        let funcao = match dono {
            DonoParametro::Funcao(fid) => self.funcao_do_no(unidade, fid),
            DonoParametro::Construtor(mid) => self.construtor_do_no(unidade, mid),
        };
        let Some(funcao) = funcao else {
            return Ok(saida);
        };
        // As sobrescritas de um método de instância têm o mesmo nomeado.
        let fe = self.programa().function(funcao);
        let mut funcoes = vec![funcao];
        if fe.kind == FunctionKind::Function
            && !fe.static_
            && let Some(c) = fe.class
        {
            let nome = self.nome(fe.name).to_string();
            let fam = self.familia(Dono::Classe(c), &nome, false, recusar_externos)?;
            let mut outras: Vec<FunctionElementId> =
                fam.funcoes.into_iter().filter(|f| *f != funcao).collect();
            outras.sort();
            for f in outras {
                let FunctionRef::Function { unit, function } = self.programa().function(f).node
                else {
                    continue;
                };
                let ast = &self.programa().unit(unit).ast;
                let homonimo = ast
                    .function(function)
                    .parameters
                    .iter()
                    .flatten()
                    .find(|q| {
                        q.kind == ast::ParameterKind::Named
                            && q.name.is_some_and(|n| n.sym == rotulo)
                    });
                if let Some(n) = homonimo.and_then(|q| q.name) {
                    funcoes.push(f);
                    if self
                        .bibliotecas
                        .contains(&self.programa().unit(unit).library)
                    {
                        usos_de(unit, n.span.start, &mut saida);
                    }
                }
            }
        }
        self.rotulos_de_argumento(&funcoes, Some(rotulo), &mut |u, s| saida.push((u, s)));
        Ok(saida)
    }

    /// Membro: as declarações da família, os usos resolvidos para ela e,
    /// num campo, os `this.x`, os inicializadores e os rótulos nomeados.
    fn ocorrencias_de_membro(
        &self,
        dono: Dono,
        nome: &str,
        estatico: bool,
        recusar_externos: bool,
        por: &mut impl FnMut(UnitId, Span),
    ) -> Result<(), String> {
        let fam = self.familia(dono, nome, estatico, recusar_externos)?;
        for f in &fam.funcoes {
            if let Some((u, s)) = self.nome_da_funcao(*f)
                && self.bibliotecas.contains(&self.programa().unit(u).library)
            {
                por(u, s);
            }
        }
        for v in &fam.variaveis {
            if let Some((u, s)) = self.nome_da_variavel(*v)
                && self.bibliotecas.contains(&self.programa().unit(u).library)
            {
                por(u, s);
            }
        }
        for u in self.unidades() {
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            let ast = &self.programa().unit(u).ast;
            for (i, e) in ast.exprs.iter().enumerate() {
                let membro = match corpos.get_resolved(ast::ExprId(i as u32)) {
                    Some(Resolved::Member {
                        member: MemberRef::Function(f),
                        ..
                    })
                    | Some(Resolved::ExtensionMember { member: f, .. }) => fam.funcoes.contains(f),
                    Some(Resolved::Member {
                        member: MemberRef::Variable(v),
                        ..
                    }) => fam.variaveis.contains(v),
                    _ => false,
                };
                if !membro {
                    continue;
                }
                match &e.kind {
                    ExprKind::Identifier(n) => por(u, n.span),
                    ExprKind::Property { name, .. } => por(u, name.span),
                    _ => {}
                }
            }
        }
        // Parâmetros `this.x`, inicializadores `x = e` e os rótulos dos
        // `this.x` nomeados nas chamadas dos construtores.
        if !estatico {
            let mut construtores_nomeados = Vec::new();
            let mut simbolo = None;
            for c in &fam.classes {
                for (u, m) in self.programa().membros_da_classe(*c) {
                    let MemberKind::Constructor(k) = &self.programa().unit(u).ast.member(m).kind
                    else {
                        continue;
                    };
                    for p in k.parameters.iter() {
                        if p.this_
                            && let Some(n) = p.name
                            && self.nome(n.sym) == nome
                        {
                            por(u, n.span);
                            simbolo = Some(n.sym);
                            if p.kind == ast::ParameterKind::Named
                                && p.public_name.is_none()
                                && let Some(f) = self.construtor_do_no(u, m)
                            {
                                construtores_nomeados.push(f);
                            }
                        }
                    }
                    for i in k.initializers.iter() {
                        if let ast::Initializer::Field { name, .. } = i
                            && self.nome(name.sym) == nome
                        {
                            por(u, name.span);
                        }
                    }
                }
            }
            if !construtores_nomeados.is_empty() {
                self.rotulos_de_argumento(&construtores_nomeados, simbolo, por);
            }
        }
        Ok(())
    }

    /// Construtor nomeado: a declaração, as criações (`A.nome()`,
    /// `new A.nome()`), os tear-offs, os redirecionamentos (`this.nome()`,
    /// `super.nome()`, `= A.nome`), as constantes de enum e os metadados.
    fn ocorrencias_de_construtor(&self, f: FunctionElementId, por: &mut impl FnMut(UnitId, Span)) {
        let p = self.programa();
        if let Some((u, s)) = self.nome_da_funcao(f) {
            por(u, s);
        }
        let Some(classe) = p.function(f).class else {
            return;
        };
        let simbolo = p.function(f).name;
        for u in self.unidades() {
            let unidade = p.unit(u);
            let ast = &unidade.ast;
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                let id = ast::ExprId(i as u32);
                let deste = matches!(corpos.get_resolved(id), Some(Resolved::Constructor(g)) if *g == f)
                    || corpos.get_resolved(id).is_none()
                        && construtor_da_chamada(ast, corpos, id) == Some(f);
                if !deste {
                    continue;
                }
                match &e.kind {
                    ExprKind::Property { name, .. } if name.sym == simbolo => por(u, name.span),
                    ExprKind::InstanceCreation {
                        constructor: Some(n),
                        ..
                    } if n.sym == simbolo => por(u, n.span),
                    // `new A.nome()`: o nome do construtor vem no tipo.
                    ExprKind::InstanceCreation {
                        constructor: None,
                        ty,
                        ..
                    } => {
                        if let Some(n) = self.construtor_no_tipo(u, *ty, classe, simbolo) {
                            por(u, n);
                        }
                    }
                    _ => {}
                }
            }
            for (mi, m) in ast.members.iter().enumerate() {
                let MemberKind::Constructor(k) = &m.kind else {
                    continue;
                };
                // A classe dona deste construtor, para `this.`/`super.`.
                let dono = self
                    .construtor_do_no(u, ast::MemberId(mi as u32))
                    .and_then(|g| p.function(g).class);
                for ini in k.initializers.iter() {
                    let (alvo_classe, n) = match ini {
                        ast::Initializer::Redirect {
                            constructor: Some(n),
                            ..
                        } => (dono, n),
                        ast::Initializer::Super {
                            constructor: Some(n),
                            ..
                        } => (dono.and_then(|d| p.class(d).supertype_class), n),
                        _ => continue,
                    };
                    if alvo_classe == Some(classe) && n.sym == simbolo {
                        por(u, n.span);
                    }
                }
                if let Some(r) = &k.redirect {
                    match r.constructor {
                        Some(n)
                            if n.sym == simbolo && self.classe_do_tipo(u, r.ty) == Some(classe) =>
                        {
                            por(u, n.span)
                        }
                        None => {
                            if let Some(n) = self.construtor_no_tipo(u, r.ty, classe, simbolo) {
                                por(u, n);
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Constantes de enum `a.nome(...)`.
            for d in &ast.decls {
                if let DeclKind::Enum(e) = &d.kind {
                    let eh_dono = p
                        .class(classe)
                        .decl
                        .is_some_and(|dr| dr.unit == u && std::ptr::eq(ast.decl(dr.decl), d));
                    if !eh_dono {
                        continue;
                    }
                    for c in &e.constants {
                        if let Some(n) = c.constructor
                            && n.sym == simbolo
                        {
                            por(u, n.span);
                        }
                    }
                }
            }
            // Metadados `@A.nome(...)` e `@p.A.nome(...)`.
            for a in metadados(ast, &unidade.unit) {
                let partes: Vec<SymbolId> = a.name.iter().map(|n| n.sym).collect();
                if partes.len() >= 2
                    && let Some((Alvo::Construtor(g), _)) =
                        self.resolver_caminho(unidade.library, None, &partes)
                    && g == f
                {
                    por(u, a.name[a.name.len() - 1].span);
                }
            }
        }
    }

    /// `A.nome` escrito como tipo (`new A.nome()`, `= A.nome`) quando `A` é
    /// `classe` (e não um prefixo) e `nome` é `simbolo`: o span de `nome`.
    fn construtor_no_tipo(
        &self,
        u: UnitId,
        ty: ast::TypeId,
        classe: ClassId,
        simbolo: SymbolId,
    ) -> Option<Span> {
        let p = self.programa();
        let unidade = p.unit(u);
        let ast::TypeKind::Named { name, .. } = &unidade.ast.ty(ty).kind else {
            return None;
        };
        let [a, n] = &name[..] else { return None };
        if n.sym != simbolo || eh_prefixo(p, unidade.library, a.sym) {
            return None;
        }
        match p.lookup(unidade.library, a.sym)?.getter? {
            Element::Class(c) if c == classe => Some(n.span),
            _ => None,
        }
    }

    /// A classe de uma anotação de tipo `A` ou `p.A` escrita em `u`.
    pub(crate) fn classe_do_tipo(&self, u: UnitId, ty: ast::TypeId) -> Option<ClassId> {
        let p = self.programa();
        let unidade = p.unit(u);
        let ast::TypeKind::Named { name, .. } = &unidade.ast.ty(ty).kind else {
            return None;
        };
        let vinculo = match &name[..] {
            [n] => p.lookup(unidade.library, n.sym),
            [pr, n] => p.lookup_prefixed(unidade.library, pr.sym, n.sym),
            _ => None,
        }?;
        match vinculo.getter? {
            Element::Class(c) => Some(c),
            _ => None,
        }
    }

    /// Prefixo de import: o `as p` de cada import da biblioteca e cada `p.`
    /// escrito nela (expressões, tipos, metadados).
    fn ocorrencias_de_prefixo(
        &self,
        lib: LibraryId,
        nome: SymbolId,
        por: &mut impl FnMut(UnitId, Span),
    ) {
        let p = self.programa();
        for i in &p.library(lib).imports {
            if i.prefix == Some(nome)
                && let Some(d) = p.unit(i.unit).unit.directives.get(i.directive)
                && let ast::DirectiveKind::Import {
                    prefix: Some(n), ..
                } = &d.kind
            {
                por(i.unit, n.span);
            }
        }
        if !self.bibliotecas.contains(&lib) {
            return;
        }
        for &u in &p.library(lib).units {
            let unidade = p.unit(u);
            let ast = &unidade.ast;
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                if let ExprKind::Identifier(n) = &e.kind
                    && n.sym == nome
                    && matches!(
                        corpos.get_resolved(ast::ExprId(i as u32)),
                        Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))
                    )
                {
                    por(u, n.span);
                }
            }
            for t in &ast.types {
                if let ast::TypeKind::Named { name, .. } = &t.kind
                    && let [pr, _] = &name[..]
                    && pr.sym == nome
                {
                    por(u, pr.span);
                }
            }
            for a in metadados(ast, &unidade.unit) {
                if a.name.len() >= 2 && a.name[0].sym == nome && eh_prefixo(p, lib, nome) {
                    por(u, a.name[0].span);
                }
            }
        }
    }

    /// Parâmetro de tipo: a declaração e os usos (tipos e expressões) cujo
    /// parâmetro mais interno com esse nome é ele.
    fn ocorrencias_de_parametro_de_tipo(
        &self,
        unidade: UnitId,
        declaracao: usize,
        por: &mut impl FnMut(UnitId, Span),
    ) {
        let u = self.programa().unit(unidade);
        let ast = &u.ast;
        let Some(nome) = palavra(&u.source, declaracao) else {
            return;
        };
        por(unidade, nome);
        let texto = &u.source[nome.start..nome.end];
        for t in &ast.types {
            if let ast::TypeKind::Named { name, .. } = &t.kind
                && let [n] = &name[..]
                && self.nome(n.sym) == texto
                && declaracao_de_parametro_de_tipo(ast, n.span.start, n.sym) == Some(declaracao)
            {
                por(unidade, n.span);
            }
        }
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        for (i, e) in ast.exprs.iter().enumerate() {
            if let ExprKind::Identifier(n) = &e.kind
                && self.nome(n.sym) == texto
                && matches!(
                    corpos.get_resolved(ast::ExprId(i as u32)),
                    Some(Resolved::TypeParameter(_))
                )
                && declaracao_de_parametro_de_tipo(ast, n.span.start, n.sym) == Some(declaracao)
            {
                por(unidade, n.span);
            }
        }
    }

    /// Rótulos `nome:` dos argumentos nas chamadas de `funcoes`.
    pub(crate) fn rotulos_de_argumento(
        &self,
        funcoes: &[FunctionElementId],
        rotulo: Option<SymbolId>,
        por: &mut impl FnMut(UnitId, Span),
    ) {
        let Some(rotulo) = rotulo else { return };
        for u in self.unidades() {
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            let ast = &self.programa().unit(u).ast;
            for (i, e) in ast.exprs.iter().enumerate() {
                let (argumentos, alvo) = match &e.kind {
                    ExprKind::Call { target, arguments } => (&**arguments, Some(*target)),
                    ExprKind::InstanceCreation { arguments, .. } => (&**arguments, None),
                    _ => continue,
                };
                let chamada = [Some(ast::ExprId(i as u32)), alvo]
                    .into_iter()
                    .flatten()
                    .find_map(|x| match corpos.get_resolved(x) {
                        Some(Resolved::Constructor(f))
                        | Some(Resolved::Element(Element::Function(f)))
                        | Some(Resolved::Member {
                            member: MemberRef::Function(f),
                            ..
                        })
                        | Some(Resolved::ExtensionMember { member: f, .. }) => Some(*f),
                        _ => None,
                    });
                if !chamada.is_some_and(|f| funcoes.contains(&f)) {
                    continue;
                }
                for a in argumentos.args.iter() {
                    if let Some(n) = a.name
                        && n.sym == rotulo
                    {
                        por(u, n.span);
                    }
                }
            }
        }
    }

    /// Elementos que um alvo de topo abrange: getter e setter homônimos, a
    /// variável e os seus acessores.
    pub(crate) fn elementos_de_topo(&self, el: Element) -> Vec<Element> {
        let p = self.programa();
        let mut v = vec![el];
        let (lib, nome) = match el {
            Element::Variable(x) => (p.variable(x).library, p.variable(x).name),
            Element::Function(f) => (p.function(f).library, p.function(f).name),
            _ => return v,
        };
        if let Some(b) = p.library(lib).declared.get(&nome) {
            v.extend(b.getter);
            v.extend(b.setter);
        }
        for e in v.clone() {
            if let Element::Variable(x) = e {
                v.extend(p.variable(x).getter.map(Element::Function));
                v.extend(p.variable(x).setter.map(Element::Function));
            }
        }
        v.sort_by_key(|e| format!("{e:?}"));
        v.dedup();
        v
    }

    /// Topo: declarações, usos, tipos, metadados e `show`/`hide`.
    fn ocorrencias_de_topo(&self, el: Element, por: &mut impl FnMut(UnitId, Span)) {
        let p = self.programa();
        let elementos = self.elementos_de_topo(el);
        let Some(simbolo) = self.consulta.nome_do_elemento(el) else {
            return;
        };
        // Declarações.
        for e in &elementos {
            if let Some((u, s)) = self.nome_do_elemento_de_topo(*e) {
                por(u, s);
            }
            // Construtores escritos com o nome da classe.
            if let Element::Class(c) = e {
                for (u, m) in p.membros_da_classe(*c) {
                    if let MemberKind::Constructor(k) = &p.unit(u).ast.member(m).kind
                        && k.class_name.sym == simbolo
                    {
                        por(u, k.class_name.span);
                    }
                }
            }
        }
        for u in self.unidades() {
            let unidade = p.unit(u);
            let ast = &unidade.ast;
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            // Usos em expressões.
            for (i, e) in ast.exprs.iter().enumerate() {
                let r = match corpos.get_resolved(ast::ExprId(i as u32)) {
                    Some(Resolved::Element(r)) => *r,
                    _ => continue,
                };
                if !elementos.contains(&r) {
                    continue;
                }
                match &e.kind {
                    ExprKind::Identifier(n) => por(u, n.span),
                    ExprKind::Property { name, .. } => por(u, name.span),
                    _ => {}
                }
            }
            // Anotações de tipo.
            for t in &ast.types {
                let ast::TypeKind::Named { name, .. } = &t.kind else {
                    continue;
                };
                let vinculo = match &name[..] {
                    [n] if n.sym == simbolo
                        && declaracao_de_parametro_de_tipo(ast, n.span.start, n.sym).is_none() =>
                    {
                        p.lookup(unidade.library, n.sym).map(|b| (b, *n))
                    }
                    [pr, n] if n.sym == simbolo => p
                        .lookup_prefixed(unidade.library, pr.sym, n.sym)
                        .map(|b| (b, *n)),
                    _ => None,
                };
                if let Some((b, n)) = vinculo
                    && b.getter.is_some_and(|g| elementos.contains(&g))
                {
                    por(u, n.span);
                }
            }
            // Metadados (`@nome`, `@p.nome`, `@Classe(...)`).
            for a in metadados(ast, &unidade.unit) {
                let n = match &a.name[..] {
                    [n, ..] if n.sym == simbolo => {
                        p.lookup(unidade.library, n.sym).map(|b| (b, *n))
                    }
                    [pr, n, ..] if n.sym == simbolo => p
                        .lookup_prefixed(unidade.library, pr.sym, n.sym)
                        .map(|b| (b, *n)),
                    _ => None,
                };
                if let Some((b, n)) = n
                    && b.getter.is_some_and(|g| elementos.contains(&g))
                {
                    por(u, n.span);
                }
            }
        }
        // `show`/`hide` de imports e exports que alcançam o elemento.
        for lib in &self.bibliotecas {
            let l = p.library(*lib);
            let diretivas = l
                .imports
                .iter()
                .map(|i| (i.unit, i.directive, i.library))
                .chain(l.exports.iter().map(|e| (e.unit, e.directive, e.library)));
            for (u, d, alvo) in diretivas {
                let Some(dir) = p.unit(u).unit.directives.get(d) else {
                    continue;
                };
                let combinadores = match &dir.kind {
                    ast::DirectiveKind::Import { combinators, .. }
                    | ast::DirectiveKind::Export { combinators, .. } => combinators,
                    _ => continue,
                };
                let exporta = p
                    .library(alvo)
                    .exported
                    .get(&simbolo)
                    .and_then(|b| b.getter.or(b.setter))
                    .is_some_and(|g| elementos.contains(&g));
                if !exporta {
                    continue;
                }
                for c in combinadores {
                    let (ast::Combinator::Show(nomes) | ast::Combinator::Hide(nomes)) = c;
                    for n in nomes {
                        if n.sym == simbolo {
                            por(u, n.span);
                        }
                    }
                }
            }
        }
    }

    /// Superclasses, mixins, interfaces e `on` de `c`, transitivos.
    pub(crate) fn supertipos(&self, c: ClassId) -> HashSet<ClassId> {
        let mut vistos = HashSet::new();
        let mut pilha = vec![c];
        while let Some(x) = pilha.pop() {
            let cl = self.programa().class(x);
            for s in cl
                .supertype_class
                .iter()
                .chain(&cl.mixin_classes)
                .chain(&cl.interface_classes)
                .chain(&cl.on_classes)
            {
                if vistos.insert(*s) {
                    pilha.push(*s);
                }
            }
        }
        vistos
    }

    /// Membros de `c` com nome base `nome` (de instância ou estáticos).
    pub(crate) fn declarados(
        &self,
        c: ClassId,
        nome: &str,
        estatico: bool,
    ) -> Vec<FunctionElementId> {
        let cl = self.programa().class(c);
        let mapa = if estatico {
            &cl.static_members
        } else {
            &cl.instance_members
        };
        let mut v: Vec<FunctionElementId> = mapa
            .iter()
            .filter(|(s, _)| nome_base(self.nome(**s)) == nome)
            .map(|(_, f)| *f)
            .collect();
        v.sort();
        v
    }

    /// Erro quando `lib` não é do projeto (renomear só mexe no projeto).
    fn recusar_externo(&self, lib: LibraryId, nome: &str) -> Result<(), String> {
        if self.do_projeto(lib) {
            return Ok(());
        }
        let uri = &self.programa().library(lib).uri;
        Err(format!(
            "'{nome}' é declarado em {uri}, fora do projeto; só declarações do projeto podem ser renomeadas."
        ))
    }

    /// A família de um membro: funções, variáveis e classes. Membros de
    /// instância sobem e descem pela hierarquia até fechar; estáticos e
    /// membros de extensão são só os do dono.
    ///
    /// # Erros
    ///
    /// Com `recusar_externos`, quando algum membro da família é declarado
    /// fora do projeto (SDK ou pacote).
    pub(crate) fn familia(
        &self,
        dono: Dono,
        nome: &str,
        estatico: bool,
        recusar_externos: bool,
    ) -> Result<Familia, String> {
        let p = self.programa();
        let recusar = |lib: LibraryId| {
            if recusar_externos {
                self.recusar_externo(lib, nome)
            } else {
                Ok(())
            }
        };
        let mut funcoes = HashSet::new();
        let mut variaveis = HashSet::new();
        let classes: Vec<ClassId> = match dono {
            Dono::Extensao(x) => {
                let ext = p.extension(x);
                recusar(ext.library)?;
                let mapa = if estatico {
                    &ext.static_members
                } else {
                    &ext.instance_members
                };
                for (s, f) in mapa {
                    if nome_base(self.nome(*s)) == nome {
                        funcoes.insert(*f);
                    }
                }
                for v in &ext.fields {
                    if self.nome(p.variable(*v).name) == nome {
                        variaveis.insert(*v);
                    }
                }
                Vec::new()
            }
            Dono::Classe(c) if estatico => {
                recusar(p.class(c).library)?;
                funcoes.extend(self.declarados(c, nome, true));
                for v in p.class(c).fields.iter().chain(&p.class(c).enum_constants) {
                    let ve = p.variable(*v);
                    if ve.static_ && self.nome(ve.name) == nome {
                        variaveis.insert(*v);
                    }
                }
                vec![c]
            }
            Dono::Classe(c) => {
                let declara = |x: ClassId| !self.declarados(x, nome, false).is_empty();
                let mut raizes: HashSet<ClassId> = self
                    .supertipos(c)
                    .into_iter()
                    .chain([c])
                    .filter(|x| declara(*x))
                    .collect();
                let declarantes: Vec<ClassId> = (0..p.classes.len())
                    .map(|i| ClassId(i as u32))
                    .filter(|x| declara(*x))
                    .collect();
                let supers: Vec<(ClassId, HashSet<ClassId>)> = declarantes
                    .iter()
                    .map(|x| (*x, self.supertipos(*x)))
                    .collect();
                let familia = loop {
                    let familia: Vec<ClassId> = supers
                        .iter()
                        .filter(|(x, s)| raizes.contains(x) || s.iter().any(|y| raizes.contains(y)))
                        .map(|(x, _)| *x)
                        .collect();
                    let novas: HashSet<ClassId> = supers
                        .iter()
                        .filter(|(x, _)| familia.contains(x))
                        .flat_map(|(x, s)| s.iter().copied().chain([*x]))
                        .filter(|x| declara(*x))
                        .collect();
                    if novas.is_subset(&raizes) {
                        break familia;
                    }
                    raizes.extend(novas);
                };
                for x in &familia {
                    recusar(p.class(*x).library)?;
                    for f in self.declarados(*x, nome, false) {
                        funcoes.insert(f);
                        if let Some(v) = p.function(f).variable {
                            variaveis.insert(v);
                        }
                    }
                }
                familia
            }
        };
        for v in variaveis.clone() {
            funcoes.extend(p.variable(v).getter);
            funcoes.extend(p.variable(v).setter);
        }
        Ok(Familia {
            funcoes,
            variaveis,
            classes,
        })
    }
}

/// Funções, variáveis e classes de uma família de membros.
pub(crate) struct Familia {
    pub funcoes: HashSet<FunctionElementId>,
    pub variaveis: HashSet<VariableId>,
    pub classes: Vec<ClassId>,
}

/// A declaração que um comentário de documentação documenta.
#[derive(Debug, Clone, Copy)]
enum Documentado {
    Decl(ast::DeclId),
    Membro(ast::MemberId),
}

/// O construtor chamado quando `alvo` é o alvo de uma chamada resolvida
/// para construtor (`A(…)`, `A.nome(…)`, `p.A.nome(…)` sem `new`).
fn construtor_da_chamada(
    ast: &Ast,
    corpos: &dartforge_types::UnitBodyTypes,
    alvo: ast::ExprId,
) -> Option<FunctionElementId> {
    ast.exprs
        .iter()
        .enumerate()
        .find_map(|(i, e)| match &e.kind {
            ExprKind::Call { target, .. } if *target == alvo => {
                match corpos.get_resolved(ast::ExprId(i as u32)) {
                    Some(Resolved::Constructor(f)) => Some(*f),
                    _ => None,
                }
            }
            _ => None,
        })
}

/// `nome` é um prefixo de import em `lib` (e não um nome do escopo).
pub(crate) fn eh_prefixo(p: &Program, lib: LibraryId, nome: SymbolId) -> bool {
    p.library(lib).prefixes.contains_key(&nome)
        && p.lookup(lib, nome)
            .is_none_or(|b| matches!(b.getter, None | Some(Element::Prefix(..))))
}

/// Todas as anotações de metadados da unidade (declarações, membros,
/// diretivas, parâmetros, constantes de enum e parâmetros de tipo).
pub(crate) fn metadados<'a>(
    ast: &'a Ast,
    unit: &'a ast::CompilationUnit,
) -> Vec<&'a ast::Annotation> {
    let mut v: Vec<&ast::Annotation> = Vec::new();
    v.extend(unit.directives.iter().flat_map(|d| d.metadata.iter()));
    v.extend(ast.decls.iter().flat_map(|d| d.metadata.iter()));
    v.extend(ast.members.iter().flat_map(|m| m.metadata.iter()));
    for d in &ast.decls {
        if let DeclKind::Enum(e) = &d.kind {
            v.extend(e.constants.iter().flat_map(|c| c.metadata.iter()));
        }
    }
    for f in &ast.functions {
        v.extend(
            f.parameters
                .iter()
                .flatten()
                .flat_map(|p| p.metadata.iter()),
        );
    }
    for m in &ast.members {
        if let MemberKind::Constructor(k) = &m.kind {
            v.extend(k.parameters.iter().flat_map(|p| p.metadata.iter()));
        }
    }
    v
}

/// Onde um parâmetro foi declarado.
#[derive(Clone, Copy)]
pub(crate) enum DonoParametro {
    Funcao(ast::FunctionId),
    Construtor(ast::MemberId),
}

/// O parâmetro cujo nome começa em `offset`.
pub(crate) fn parametro_em(ast: &Ast, offset: usize) -> Option<(&Parameter, DonoParametro)> {
    for (i, f) in ast.functions.iter().enumerate() {
        if let Some(ps) = &f.parameters
            && let Some(p) = ps
                .iter()
                .find(|p| p.name.is_some_and(|n| n.span.start == offset))
        {
            return Some((p, DonoParametro::Funcao(ast::FunctionId(i as u32))));
        }
    }
    for (i, m) in ast.members.iter().enumerate() {
        if let MemberKind::Constructor(k) = &m.kind
            && let Some(p) = k
                .parameters
                .iter()
                .find(|p| p.name.is_some_and(|n| n.span.start == offset))
        {
            return Some((p, DonoParametro::Construtor(ast::MemberId(i as u32))));
        }
    }
    None
}

/// O offset da declaração do parâmetro de tipo `nome` mais interno em
/// escopo em `offset` (numa declaração, função ou tipo de função que o
/// contém); `None` quando nenhum está em escopo.
pub(crate) fn declaracao_de_parametro_de_tipo(
    ast: &Ast,
    offset: usize,
    nome: SymbolId,
) -> Option<usize> {
    let achar = |ps: &[ast::TypeParameter]| {
        ps.iter()
            .find(|t| t.name.sym == nome)
            .map(|t| t.name.span.start)
    };
    let mut melhor: Option<(usize, usize)> = None;
    let mut considerar = |s: Span, d: Option<usize>| {
        if let Some(d) = d
            && contem(s, offset)
            && melhor.is_none_or(|(tam, _)| s.end - s.start < tam)
        {
            melhor = Some((s.end - s.start, d));
        }
    };
    for d in &ast.decls {
        let ps = match &d.kind {
            DeclKind::Class(c) => &c.type_params,
            DeclKind::Mixin(m) => &m.type_params,
            DeclKind::Enum(e) => &e.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Typedef(t) => &t.type_params,
            _ => continue,
        };
        considerar(d.span, achar(ps));
    }
    for f in &ast.functions {
        considerar(f.span, achar(&f.type_params));
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind {
            considerar(t.span, achar(type_params));
        }
    }
    melhor.map(|(_, d)| d)
}

/// O escopo léxico de um local declarado em `offset`: o bloco, o `for`, a
/// cláusula `catch` ou a função mais interno que o contém. O bloco do corpo
/// de uma função é o mesmo escopo dos parâmetros (no Dart, `var x` no corpo
/// de `f(int x)` é declaração duplicada), então conta como a função.
pub(crate) fn escopo_do_local(ast: &Ast, offset: usize) -> Span {
    let mut corpos: HashSet<(usize, usize)> = ast
        .functions
        .iter()
        .filter_map(|f| match &f.body {
            ast::FunctionBody::Block(s) => Some(ast.stmt(*s).span),
            _ => None,
        })
        .map(|s| (s.start, s.end))
        .collect();
    for m in &ast.members {
        if let MemberKind::Constructor(k) = &m.kind
            && let ast::FunctionBody::Block(s) = &k.body
        {
            let s = ast.stmt(*s).span;
            corpos.insert((s.start, s.end));
        }
    }
    let mut candidatos: Vec<Span> = Vec::new();
    for s in &ast.stmts {
        match &s.kind {
            StmtKind::Block(_) if corpos.contains(&(s.span.start, s.span.end)) => {}
            StmtKind::Block(_) | StmtKind::For { .. } | StmtKind::ForIn { .. } => {
                candidatos.push(s.span)
            }
            StmtKind::Try { catches, .. } => candidatos.extend(catches.iter().map(|c| c.span)),
            _ => {}
        }
    }
    candidatos.extend(ast.functions.iter().map(|f| f.span));
    candidatos.extend(
        ast.members
            .iter()
            .filter(|m| matches!(m.kind, MemberKind::Constructor(_)))
            .map(|m| m.span),
    );
    candidatos
        .into_iter()
        .filter(|s| contem(*s, offset))
        .min_by_key(|s| s.end - s.start)
        .unwrap_or(Span {
            start: 0,
            end: usize::MAX,
        })
}
