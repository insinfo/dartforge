//! `textDocument/prepareRename` e `textDocument/rename`.
//!
//! O projeto inteiro é carregado (o diretório com o `pubspec.yaml` que
//! contém o arquivo, com os textos vigentes dos documentos abertos) e os
//! corpos das suas bibliotecas são inferidos pela inferência comum de
//! `crates/types`, com o registro de declarações de locais ligado. O que o
//! cursor denota vem das tabelas laterais (`get_resolved`,
//! `declaracao_local`), não de uma resolução própria:
//!
//! * local, parâmetro ou função local: a declaração e as expressões que a
//!   referem (e os rótulos de argumento, num parâmetro nomeado);
//! * membro de classe: a família de declarações ligadas por sobrescrita na
//!   hierarquia (para cima e para baixo), getter e setter juntos, cada uso
//!   resolvido para um membro da família (inclusive via instâncias e
//!   `super`), os parâmetros `this.x` e os inicializadores `x = e`;
//! * declaração de topo: a declaração, os usos resolvidos, as anotações de
//!   tipo, os construtores da classe, os `show`/`hide` e as anotações.
//!
//! Recusa nomes inválidos (palavra reservada, forma de identificador),
//! elementos declarados no SDK ou fora do projeto (inclusive membros que
//! sobrescrevem um deles), e conflitos evidentes (nome já declarado no mesmo
//! escopo, uso que passaria a ser sombreado, nome público que viraria
//! privado com usos em outra biblioteca).

use crate::DocumentStore;
use crate::consulta::Consulta;
use crate::semantica::AnalisadorSemantico;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, LibraryId,
    Program, UnitId, VariableId, VariableRef,
};
use dartforge_frontend::ast::{self, Ast, DeclKind, ExprKind, MemberKind, Parameter};
use dartforge_types::{MemberRef, Resolved};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use url::Url;

/// Teto de arquivos `.dart` varridos na raiz do projeto.
const TETO_ARQUIVOS: usize = 20_000;

/// Palavras reservadas do Dart: nunca servem de identificador.
const RESERVADAS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
    "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
    "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with",
];

/// Identificadores embutidos: servem de nome de variável e membro, mas não
/// de nome de tipo.
const EMBUTIDAS: &[&str] = &[
    "abstract",
    "as",
    "covariant",
    "deferred",
    "dynamic",
    "export",
    "extension",
    "external",
    "factory",
    "Function",
    "get",
    "implements",
    "import",
    "interface",
    "late",
    "library",
    "mixin",
    "operator",
    "part",
    "required",
    "set",
    "static",
    "typedef",
];

/// Uma edição de texto: substituir `span` (bytes) de `uri` por `texto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edicao {
    pub uri: String,
    pub span: Span,
    pub texto: String,
}

/// O projeto carregado para renomear.
pub(crate) struct Projeto {
    consulta: Consulta,
    raiz: PathBuf,
    /// Bibliotecas do projeto (com unidades sob a raiz), inferidas.
    bibliotecas: HashSet<LibraryId>,
}

/// O que se renomeia.
#[derive(Debug, Clone)]
enum Alvo {
    /// Local, parâmetro ou função local: unidade e offset do nome declarado.
    Local { unidade: UnitId, declaracao: usize },
    /// Membro de classe ou extensão, pelo nome base (sem `_=` do setter).
    Membro {
        dono: Dono,
        nome: String,
        estatico: bool,
    },
    /// Declaração de topo.
    Topo(Element),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dono {
    Classe(ClassId),
    Extensao(ExtensionId),
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

/// Carrega o projeto que contém `uri`, com os documentos abertos nos textos
/// vigentes, e infere os corpos das bibliotecas do projeto.
pub(crate) fn carregar_projeto(
    semantico: &AnalisadorSemantico,
    documentos: &DocumentStore,
    uri: &str,
) -> Option<Projeto> {
    let sdk = semantico.sdk()?;
    let arquivo = Url::parse(uri).ok()?.to_file_path().ok()?;
    let arquivo = dartforge_elements::config::sem_verbatim(
        std::fs::canonicalize(&arquivo).unwrap_or(arquivo),
    );
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
    let lista: Vec<LibraryId> = {
        let mut l: Vec<LibraryId> = bibliotecas.iter().copied().collect();
        l.sort();
        l
    };
    let consulta = Consulta::inferir(programa, nomes, &lista, true, None);
    Some(Projeto {
        consulta,
        raiz,
        bibliotecas,
    })
}

fn dentro(caminho: &Path, raiz: &Path) -> bool {
    dartforge_elements::gerado::chave(caminho).starts_with(dartforge_elements::gerado::chave(raiz))
}

fn eh_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// O identificador que contém `offset` (ou termina nele).
fn palavra(fonte: &str, offset: usize) -> Option<Span> {
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

/// Nome base de um membro: o setter é guardado como `x_=`.
fn nome_base(nome: &str) -> &str {
    nome.strip_suffix("_=").unwrap_or(nome)
}

/// Valida o nome novo para o tipo de alvo.
fn validar(novo: &str, de_tipo: bool) -> Result<(), String> {
    let b = novo.as_bytes();
    if novo.is_empty() || b[0].is_ascii_digit() || !b.iter().all(|c| eh_ident(*c)) {
        return Err(format!("'{novo}' não é um identificador válido."));
    }
    if RESERVADAS.contains(&novo) {
        return Err(format!("O nome não pode ser a palavra reservada '{novo}'."));
    }
    if de_tipo && EMBUTIDAS.contains(&novo) {
        return Err(format!(
            "Um nome de tipo não pode ser o identificador embutido '{novo}'."
        ));
    }
    Ok(())
}

impl Projeto {
    fn programa(&self) -> &Program {
        &self.consulta.programa
    }

    fn nome(&self, s: dartforge_intern::SymbolId) -> &str {
        self.consulta.nome(s)
    }

    /// Unidades das bibliotecas do projeto, em ordem.
    fn unidades(&self) -> Vec<UnitId> {
        (0..self.programa().units.len())
            .map(|i| UnitId(i as u32))
            .filter(|u| self.bibliotecas.contains(&self.programa().unit(*u).library))
            .collect()
    }

    fn uri_da_unidade(&self, u: UnitId) -> Option<String> {
        Url::from_file_path(self.programa().unit(u).path.as_ref()?)
            .ok()
            .map(|u| u.to_string())
    }

    fn unidade_do_uri(&self, uri: &str) -> Option<UnitId> {
        let caminho = Url::parse(uri).ok()?.to_file_path().ok()?;
        let caminho = dartforge_elements::config::sem_verbatim(
            std::fs::canonicalize(&caminho).unwrap_or(caminho),
        );
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
    fn do_projeto(&self, lib: LibraryId) -> bool {
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

    fn recusar_externo(&self, lib: LibraryId, nome: &str) -> Result<(), String> {
        if self.do_projeto(lib) {
            return Ok(());
        }
        let uri = &self.programa().library(lib).uri;
        Err(format!(
            "'{nome}' é declarado em {uri}, fora do projeto; só declarações do projeto podem ser renomeadas."
        ))
    }

    // -----------------------------------------------------------------
    // O que está sob o cursor
    // -----------------------------------------------------------------

    /// O alvo sob o cursor e o intervalo do nome. `Ok(None)`: não há nome
    /// renomeável ali (espaço, palavra-chave, literal).
    fn identificar(&self, unidade: UnitId, offset: usize) -> Result<Option<(Alvo, Span)>, String> {
        let u = self.programa().unit(unidade);
        let Some(nome) = palavra(&u.source, offset) else {
            return Ok(None);
        };
        let texto = &u.source[nome.start..nome.end];
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let ast = &u.ast;
        // Referências em expressões.
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let span = match &e.kind {
                ExprKind::Identifier(n) => n.span,
                ExprKind::Property { name, .. } => name.span,
                _ => continue,
            };
            if span != nome {
                continue;
            }
            let alvo = match corpos.get_resolved(id) {
                Some(Resolved::Local(_)) => match corpos.declaracao_local(id) {
                    Some(d) => self.local_ou_campo(unidade, d),
                    None => return Ok(None),
                },
                Some(Resolved::Element(el)) => self.topo(*el)?,
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                })
                | Some(Resolved::ExtensionMember { member: f, .. }) => self.membro_de_funcao(*f),
                Some(Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                }) => self.membro_de_variavel(*v)?,
                Some(Resolved::Constructor(f)) => {
                    let fe = self.programa().function(*f);
                    match fe.class {
                        Some(c) if self.nome(self.programa().class(c).name) == texto => {
                            Alvo::Topo(Element::Class(c))
                        }
                        _ => {
                            return Err(
                                "Construtores nomeados ainda não podem ser renomeados.".into()
                            );
                        }
                    }
                }
                Some(Resolved::Prefix(_)) => {
                    return Err("Prefixos de import ainda não podem ser renomeados.".into());
                }
                Some(Resolved::TypeParameter(_)) => {
                    return Err("Parâmetros de tipo ainda não podem ser renomeados.".into());
                }
                _ => {
                    return Err(format!(
                        "'{texto}' não está resolvido; não há o que renomear."
                    ));
                }
            };
            return Ok(Some((alvo, nome)));
        }
        // Declaração de local ou parâmetro.
        if corpos.tipo_local(nome.start).is_some() {
            return Ok(Some((self.local_ou_campo(unidade, nome.start), nome)));
        }
        // Anotações de tipo.
        for t in &ast.types {
            if let ast::TypeKind::Named { name, .. } = &t.kind
                && let Some(ultimo) = name.last()
                && ultimo.span == nome
            {
                let lib = u.library;
                let vinculo = match &name[..] {
                    [n] if !parametro_de_tipo_em(ast, n.span.start, n.sym) => {
                        self.programa().lookup(lib, n.sym)
                    }
                    [p, n] => self.programa().lookup_prefixed(lib, p.sym, n.sym),
                    _ => None,
                };
                return match vinculo.and_then(|b| b.getter) {
                    Some(el) => Ok(Some((self.topo(el)?, nome))),
                    None => Ok(None),
                };
            }
        }
        // Declarações.
        if let Some(alvo) = self.declaracao_em(unidade, nome)? {
            return Ok(Some((alvo, nome)));
        }
        Ok(None)
    }

    /// Local no offset `declaracao`; parâmetro `this.x` vira o campo `x`.
    fn local_ou_campo(&self, unidade: UnitId, declaracao: usize) -> Alvo {
        let ast = &self.programa().unit(unidade).ast;
        if let Some((p, _)) = parametro_em(ast, declaracao)
            && p.this_
            && let Some(alvo) = self.campo_do_parametro(unidade, declaracao)
        {
            return alvo;
        }
        Alvo::Local {
            unidade,
            declaracao,
        }
    }

    /// O campo inicializado pelo parâmetro `this.x` declarado em `offset`.
    fn campo_do_parametro(&self, unidade: UnitId, offset: usize) -> Option<Alvo> {
        let ast = &self.programa().unit(unidade).ast;
        let (p, _) = parametro_em(ast, offset)?;
        let nome = self.nome(p.name?.sym).to_string();
        // A classe cujo construtor contém o parâmetro.
        let classe = self.programa().classes.iter().position(|c| {
            c.decl.is_some_and(|d| {
                d.unit == unidade && {
                    let s = self.programa().unit(d.unit).ast.decl(d.decl).span;
                    s.start <= offset && offset < s.end
                }
            })
        })?;
        Some(Alvo::Membro {
            dono: Dono::Classe(ClassId(classe as u32)),
            nome,
            estatico: false,
        })
    }

    fn topo(&self, el: Element) -> Result<Alvo, String> {
        let el = match el {
            Element::Function(f) => match self.programa().function(f) {
                fe if fe.kind == FunctionKind::ImplicitAccessor && fe.variable.is_some() => {
                    Element::Variable(fe.variable.unwrap())
                }
                _ => el,
            },
            Element::Prefix(..) => {
                return Err("Prefixos de import ainda não podem ser renomeados.".into());
            }
            _ => el,
        };
        Ok(Alvo::Topo(el))
    }

    fn membro_de_funcao(&self, f: FunctionElementId) -> Alvo {
        let fe = self.programa().function(f);
        let nome = nome_base(self.nome(fe.name)).to_string();
        let dono = match (fe.class, fe.extension) {
            (_, Some(x)) => Dono::Extensao(x),
            (Some(c), _) => Dono::Classe(c),
            // Função de topo resolvida como membro não acontece; por segurança, topo.
            (None, None) => return Alvo::Topo(Element::Function(f)),
        };
        Alvo::Membro {
            dono,
            nome,
            estatico: fe.static_,
        }
    }

    fn membro_de_variavel(&self, v: VariableId) -> Result<Alvo, String> {
        let ve = self.programa().variable(v);
        let nome = self.nome(ve.name).to_string();
        let dono = match (ve.class, ve.extension) {
            (_, Some(x)) => Dono::Extensao(x),
            (Some(c), _) => Dono::Classe(c),
            (None, None) => return Ok(Alvo::Topo(Element::Variable(v))),
        };
        Ok(Alvo::Membro {
            dono,
            nome,
            estatico: ve.static_,
        })
    }

    /// Declaração (de topo, membro, constante de enum) cujo nome é `nome`.
    fn declaracao_em(&self, unidade: UnitId, nome: Span) -> Result<Option<Alvo>, String> {
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
                    _ => programa
                        .library(u.library)
                        .declared
                        .get(&n.sym)
                        .and_then(|b| b.getter.or(b.setter)),
                };
                return match el {
                    Some(el) => Ok(Some(self.topo(el)?)),
                    None => Ok(None),
                };
            }
            if let DeclKind::Enum(e) = &decl.kind {
                for (i, c) in e.constants.iter().enumerate() {
                    if c.name.span == nome
                        && let Some(v) = programa.variables.iter().position(|v| {
                            v.node
                                == VariableRef::EnumConstant {
                                    unit: unidade,
                                    decl: d,
                                    index: i,
                                }
                        })
                    {
                        return Ok(Some(self.membro_de_variavel(VariableId(v as u32))?));
                    }
                }
            }
        }
        for (mi, m) in ast.members.iter().enumerate() {
            match &m.kind {
                MemberKind::Method(fid) => {
                    if ast.function(*fid).name.is_some_and(|n| n.span == nome)
                        && let Some(f) = self.funcao_do_no(unidade, *fid)
                    {
                        return Ok(Some(self.membro_de_funcao(f)));
                    }
                }
                MemberKind::Field(vl) => {
                    for (i, v) in vl.variables.iter().enumerate() {
                        if v.name.span == nome
                            && let Some(vid) = programa.variables.iter().position(|x| {
                                x.node
                                    == VariableRef::Field {
                                        unit: unidade,
                                        member: ast::MemberId(mi as u32),
                                        index: i,
                                    }
                            })
                        {
                            return Ok(Some(self.membro_de_variavel(VariableId(vid as u32))?));
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    if k.class_name.span == nome {
                        let classe = programa
                            .library(u.library)
                            .declared
                            .get(&k.class_name.sym)
                            .and_then(|b| b.getter);
                        return match classe {
                            Some(el @ Element::Class(_)) => Ok(Some(Alvo::Topo(el))),
                            _ => Ok(None),
                        };
                    }
                    if k.name.is_some_and(|n| n.span == nome) {
                        return Err("Construtores nomeados ainda não podem ser renomeados.".into());
                    }
                }
            }
        }
        let em_parametros_de_tipo = ast.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(c) => c.type_params.iter().any(|t| t.name.span == nome),
            DeclKind::Mixin(m) => m.type_params.iter().any(|t| t.name.span == nome),
            DeclKind::Enum(e) => e.type_params.iter().any(|t| t.name.span == nome),
            DeclKind::Extension(x) => x.type_params.iter().any(|t| t.name.span == nome),
            DeclKind::ExtensionType(x) => x.type_params.iter().any(|t| t.name.span == nome),
            DeclKind::Typedef(t) => t.type_params.iter().any(|t| t.name.span == nome),
            _ => false,
        }) || ast
            .functions
            .iter()
            .any(|f| f.type_params.iter().any(|t| t.name.span == nome));
        if em_parametros_de_tipo {
            return Err("Parâmetros de tipo ainda não podem ser renomeados.".into());
        }
        Ok(None)
    }

    /// Elemento de função declarado pelo nó `fid` da unidade.
    fn funcao_do_no(&self, unidade: UnitId, fid: ast::FunctionId) -> Option<FunctionElementId> {
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

    // -----------------------------------------------------------------
    // Edições
    // -----------------------------------------------------------------

    /// Todas as ocorrências do alvo, como `(unidade, span)`, ordenadas.
    fn ocorrencias(&self, alvo: &Alvo) -> Result<BTreeSet<(UnitId, usize, usize)>, String> {
        let mut saida = BTreeSet::new();
        let mut por = |u: UnitId, s: Span| {
            saida.insert((u, s.start, s.end));
        };
        match alvo {
            Alvo::Local {
                unidade,
                declaracao,
            } => {
                let u = self.programa().unit(*unidade);
                let fim = palavra(&u.source, *declaracao).map_or(*declaracao, |s| s.end);
                por(
                    *unidade,
                    Span {
                        start: *declaracao,
                        end: fim,
                    },
                );
                let corpos = &self.consulta.corpos.units[unidade.0 as usize];
                for (e, d) in &corpos.declaracoes_de_locais {
                    if d == declaracao
                        && let ExprKind::Identifier(n) = &u.ast.expr(*e).kind
                    {
                        por(*unidade, n.span);
                    }
                }
                // Parâmetro nomeado: os rótulos nas chamadas da função.
                if let Some((p, dono)) = parametro_em(&u.ast, *declaracao)
                    && p.kind == ast::ParameterKind::Named
                    && p.public_name.is_none()
                {
                    let alvo_funcao = match dono {
                        DonoParametro::Funcao(fid) => self.funcao_do_no(*unidade, fid),
                        DonoParametro::Construtor(mid) => self
                            .programa()
                            .functions
                            .iter()
                            .position(|f| {
                                f.node
                                    == FunctionRef::Constructor {
                                        unit: *unidade,
                                        member: mid,
                                    }
                            })
                            .map(|i| FunctionElementId(i as u32)),
                    };
                    if let Some(f) = alvo_funcao {
                        let rotulo = p.name.map(|n| n.sym);
                        self.rotulos_de_argumento(&[f], rotulo, &mut por);
                    }
                }
            }
            Alvo::Topo(el) => self.ocorrencias_de_topo(*el, &mut por),
            Alvo::Membro {
                dono,
                nome,
                estatico,
            } => {
                let (funcoes, variaveis, classes) = self.familia(*dono, nome, *estatico)?;
                for f in &funcoes {
                    let fe = self.programa().function(*f);
                    match fe.node {
                        FunctionRef::Function { unit, function } => {
                            if let Some(n) = self.programa().unit(unit).ast.function(function).name
                            {
                                por(unit, n.span);
                            }
                        }
                        _ => {
                            if let Some(v) = fe.variable
                                && let Some((u, s)) = self.nome_da_variavel(v)
                            {
                                por(u, s);
                            }
                        }
                    }
                }
                for v in &variaveis {
                    if let Some((u, s)) = self.nome_da_variavel(*v) {
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
                            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                                funcoes.contains(f)
                            }
                            Some(Resolved::Member {
                                member: MemberRef::Variable(v),
                                ..
                            }) => variaveis.contains(v),
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
                // Parâmetros `this.x`, inicializadores `x = e` e os rótulos
                // dos `this.x` nomeados nas chamadas dos construtores.
                if !*estatico {
                    let mut construtores_nomeados = Vec::new();
                    let mut simbolo = None;
                    for c in &classes {
                        for (u, m) in self.programa().membros_da_classe(*c) {
                            let MemberKind::Constructor(k) =
                                &self.programa().unit(u).ast.member(m).kind
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
                                        && let Some(f) =
                                            self.programa().functions.iter().position(|f| {
                                                f.node
                                                    == FunctionRef::Constructor {
                                                        unit: u,
                                                        member: m,
                                                    }
                                            })
                                    {
                                        construtores_nomeados.push(FunctionElementId(f as u32));
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
                        self.rotulos_de_argumento(&construtores_nomeados, simbolo, &mut por);
                    }
                }
            }
        }
        Ok(saida)
    }

    /// Unidade e span do nome de uma variável de topo, campo ou constante.
    fn nome_da_variavel(&self, v: VariableId) -> Option<(UnitId, Span)> {
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
            _ => None,
        }
    }

    /// Rótulos `nome:` dos argumentos nas chamadas de `funcoes`.
    fn rotulos_de_argumento(
        &self,
        funcoes: &[FunctionElementId],
        rotulo: Option<dartforge_intern::SymbolId>,
        por: &mut impl FnMut(UnitId, Span),
    ) {
        let Some(rotulo) = rotulo else { return };
        for u in self.unidades() {
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            let ast = &self.programa().unit(u).ast;
            for (i, e) in ast.exprs.iter().enumerate() {
                let (argumentos, alvo) = match &e.kind {
                    ExprKind::Call { target, arguments } => (arguments, Some(*target)),
                    ExprKind::InstanceCreation { arguments, .. } => (arguments, None),
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
    fn elementos_de_topo(&self, el: Element) -> Vec<Element> {
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

    fn ocorrencias_de_topo(&self, el: Element, por: &mut impl FnMut(UnitId, Span)) {
        let p = self.programa();
        let elementos = self.elementos_de_topo(el);
        let Some(simbolo) = self.consulta.nome_do_elemento(el) else {
            return;
        };
        // Declarações.
        for e in &elementos {
            match *e {
                Element::Class(c) => {
                    if let Some(d) = p.class(c).decl {
                        let decl = p.unit(d.unit).ast.decl(d.decl);
                        let n = match &decl.kind {
                            DeclKind::Class(x) => Some(x.name),
                            DeclKind::Mixin(x) => Some(x.name),
                            DeclKind::Enum(x) => Some(x.name),
                            DeclKind::ExtensionType(x) => Some(x.name),
                            _ => None,
                        };
                        if let Some(n) = n {
                            por(d.unit, n.span);
                        }
                    }
                    // Construtores escritos com o nome da classe.
                    for (u, m) in p.membros_da_classe(c) {
                        if let MemberKind::Constructor(k) = &p.unit(u).ast.member(m).kind
                            && k.class_name.sym == simbolo
                        {
                            por(u, k.class_name.span);
                        }
                    }
                }
                Element::Typedef(t) => {
                    let d = p.typedef(t).decl;
                    if let DeclKind::Typedef(x) = &p.unit(d.unit).ast.decl(d.decl).kind {
                        por(d.unit, x.name.span);
                    }
                }
                Element::Extension(x) => {
                    let d = p.extension(x).decl;
                    if let DeclKind::Extension(xd) = &p.unit(d.unit).ast.decl(d.decl).kind
                        && let Some(n) = xd.name
                    {
                        por(d.unit, n.span);
                    }
                }
                Element::Function(f) => {
                    if let FunctionRef::Function { unit, function } = p.function(f).node
                        && let Some(n) = p.unit(unit).ast.function(function).name
                    {
                        por(unit, n.span);
                    }
                }
                Element::Variable(v) => {
                    if let Some((u, s)) = self.nome_da_variavel(v) {
                        por(u, s);
                    }
                }
                Element::Prefix(..) => {}
            }
        }
        for u in self.unidades() {
            let unidade = p.unit(u);
            let ast = &unidade.ast;
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            // Usos em expressões.
            for (i, e) in ast.exprs.iter().enumerate() {
                let Some(Resolved::Element(r)) = corpos.get_resolved(ast::ExprId(i as u32)) else {
                    continue;
                };
                if !elementos.contains(r) {
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
                    [n] if n.sym == simbolo && !parametro_de_tipo_em(ast, n.span.start, n.sym) => {
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
            // Anotações de metadados (`@nome`, `@p.nome`, `@Classe(...)`).
            let metadados = ast
                .decls
                .iter()
                .flat_map(|d| d.metadata.iter())
                .chain(ast.members.iter().flat_map(|m| m.metadata.iter()));
            for a in metadados {
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
    fn supertipos(&self, c: ClassId) -> HashSet<ClassId> {
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

    /// Membros de instância de `c` com nome base `nome`.
    fn declarados(&self, c: ClassId, nome: &str, estatico: bool) -> Vec<FunctionElementId> {
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

    /// A família de um membro: funções, variáveis e classes. Membros de
    /// instância sobem e descem pela hierarquia até fechar; estáticos e
    /// membros de extensão são só os do dono.
    #[allow(clippy::type_complexity)]
    fn familia(
        &self,
        dono: Dono,
        nome: &str,
        estatico: bool,
    ) -> Result<
        (
            HashSet<FunctionElementId>,
            HashSet<VariableId>,
            Vec<ClassId>,
        ),
        String,
    > {
        let p = self.programa();
        let mut funcoes = HashSet::new();
        let mut variaveis = HashSet::new();
        let classes: Vec<ClassId> = match dono {
            Dono::Extensao(x) => {
                let ext = p.extension(x);
                self.recusar_externo(ext.library, nome)?;
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
                self.recusar_externo(p.class(c).library, nome)?;
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
                    self.recusar_externo(p.class(*x).library, nome)?;
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
        Ok((funcoes, variaveis, classes))
    }

    // -----------------------------------------------------------------
    // Conflitos
    // -----------------------------------------------------------------

    fn conflitos(&self, alvo: &Alvo, antigo: &str, novo: &str) -> Result<(), String> {
        let p = self.programa();
        match alvo {
            Alvo::Local {
                unidade,
                declaracao,
            } => {
                let u = p.unit(*unidade);
                let corpos = &self.consulta.corpos.units[unidade.0 as usize];
                // O corpo (função ou construtor) mais interno que contém a declaração.
                let regiao = regiao_de(&u.ast, *declaracao);
                for &offset in corpos.tipos_de_locais.keys() {
                    if offset != *declaracao
                        && regiao.start <= offset
                        && offset < regiao.end
                        && palavra(&u.source, offset)
                            .is_some_and(|s| &u.source[s.start..s.end] == novo)
                    {
                        return Err(format!("Já existe um local chamado '{novo}' neste corpo."));
                    }
                }
                for (i, e) in u.ast.exprs.iter().enumerate() {
                    if let ExprKind::Identifier(n) = &e.kind
                        && regiao.start <= n.span.start
                        && n.span.end <= regiao.end
                        && self.nome(n.sym) == novo
                        && corpos.declaracao_local(ast::ExprId(i as u32)).is_none()
                    {
                        return Err(format!(
                            "O uso de '{novo}' neste corpo passaria a ser sombreado pelo local renomeado."
                        ));
                    }
                }
            }
            Alvo::Membro {
                dono,
                nome,
                estatico,
            } => {
                let (_, _, classes) = self.familia(*dono, nome, *estatico)?;
                for c in classes {
                    let cl = p.class(c);
                    let ja = self.supertipos(c).into_iter().chain([c]).any(|x| {
                        let cx = p.class(x);
                        cx.instance_members
                            .keys()
                            .chain(cx.static_members.keys())
                            .any(|s| nome_base(self.nome(*s)) == novo)
                    });
                    if ja {
                        return Err(format!(
                            "A classe '{}' já tem um membro chamado '{novo}'.",
                            self.nome(cl.name)
                        ));
                    }
                }
                if let Dono::Extensao(x) = dono {
                    let ext = p.extension(*x);
                    if ext
                        .instance_members
                        .keys()
                        .chain(ext.static_members.keys())
                        .any(|s| nome_base(self.nome(*s)) == novo)
                    {
                        return Err(format!("A extensão já tem um membro chamado '{novo}'."));
                    }
                }
                self.privacidade(antigo, novo, alvo)?;
            }
            Alvo::Topo(el) => {
                let lib = match *el {
                    Element::Class(c) => p.class(c).library,
                    Element::Extension(x) => p.extension(x).library,
                    Element::Typedef(t) => p.typedef(t).library,
                    Element::Function(f) => p.function(f).library,
                    Element::Variable(v) => p.variable(v).library,
                    Element::Prefix(l, _) => l,
                };
                let ja = |l: LibraryId| p.library(l).declared.keys().any(|s| self.nome(*s) == novo);
                if ja(lib) {
                    return Err(format!("A biblioteca já declara '{novo}'."));
                }
                let usos = self.ocorrencias(alvo)?;
                for (u, _, _) in &usos {
                    let l = p.unit(*u).library;
                    if l != lib && ja(l) {
                        return Err(format!(
                            "'{novo}' já é declarado em {}, que usa '{antigo}'.",
                            p.library(l).uri
                        ));
                    }
                }
                self.privacidade(antigo, novo, alvo)?;
            }
        }
        Ok(())
    }

    /// Público que vira privado: recusa se há usos em outra biblioteca.
    fn privacidade(&self, antigo: &str, novo: &str, alvo: &Alvo) -> Result<(), String> {
        if antigo.starts_with('_') || !novo.starts_with('_') {
            return Ok(());
        }
        let usos = self.ocorrencias(alvo)?;
        let libs: HashSet<LibraryId> = usos
            .iter()
            .map(|(u, _, _)| self.programa().unit(*u).library)
            .collect();
        if libs.len() > 1 {
            return Err(format!(
                "'{novo}' seria privado, mas '{antigo}' é usado em outras bibliotecas."
            ));
        }
        Ok(())
    }
}

/// O corpo (função, método ou construtor) mais interno que contém `offset`.
fn regiao_de(ast: &Ast, offset: usize) -> Span {
    let funcoes = ast.functions.iter().map(|f| f.span);
    let construtores = ast
        .members
        .iter()
        .filter(|m| matches!(m.kind, MemberKind::Constructor(_)))
        .map(|m| m.span);
    funcoes
        .chain(construtores)
        .filter(|s| s.start <= offset && offset < s.end)
        .min_by_key(|s| s.end - s.start)
        .unwrap_or(Span {
            start: 0,
            end: usize::MAX,
        })
}

/// Onde um parâmetro foi declarado.
#[derive(Clone, Copy)]
enum DonoParametro {
    Funcao(ast::FunctionId),
    Construtor(ast::MemberId),
}

/// O parâmetro cujo nome começa em `offset`.
fn parametro_em(ast: &Ast, offset: usize) -> Option<(&Parameter, DonoParametro)> {
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

/// Um parâmetro de tipo chamado `nome` está em escopo em `offset` (numa
/// declaração, função ou tipo de função que o contém).
fn parametro_de_tipo_em(ast: &Ast, offset: usize, nome: dartforge_intern::SymbolId) -> bool {
    let tem = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.sym == nome);
    let dentro = |s: Span| s.start <= offset && offset < s.end;
    ast.decls.iter().any(|d| {
        dentro(d.span)
            && match &d.kind {
                DeclKind::Class(c) => tem(&c.type_params),
                DeclKind::Mixin(m) => tem(&m.type_params),
                DeclKind::Enum(e) => tem(&e.type_params),
                DeclKind::Extension(x) => tem(&x.type_params),
                DeclKind::ExtensionType(x) => tem(&x.type_params),
                DeclKind::Typedef(t) => tem(&t.type_params),
                _ => false,
            }
    }) || ast
        .functions
        .iter()
        .any(|f| dentro(f.span) && tem(&f.type_params)) || ast.types.iter().any(|t| {
        dentro(t.span)
            && matches!(&t.kind, ast::TypeKind::Function { type_params, .. } if tem(type_params))
    })
}

/// `prepareRename`: o intervalo do nome e o texto atual, `Ok(None)` quando
/// não há nome renomeável sob o cursor.
pub(crate) fn preparar(
    projeto: &Projeto,
    uri: &str,
    offset: usize,
) -> Result<Option<(Span, String)>, String> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Ok(None);
    };
    let Some((alvo, span)) = projeto.identificar(unidade, offset)? else {
        return Ok(None);
    };
    // Recusa cedo o que está fora do projeto.
    projeto.ocorrencias(&alvo)?;
    if let Alvo::Topo(el) = alvo {
        let lib = match el {
            Element::Class(c) => projeto.programa().class(c).library,
            Element::Extension(x) => projeto.programa().extension(x).library,
            Element::Typedef(t) => projeto.programa().typedef(t).library,
            Element::Function(f) => projeto.programa().function(f).library,
            Element::Variable(v) => projeto.programa().variable(v).library,
            Element::Prefix(l, _) => l,
        };
        let fonte = &projeto.programa().unit(unidade).source;
        projeto.recusar_externo(lib, &fonte[span.start..span.end])?;
    }
    let fonte = &projeto.programa().unit(unidade).source;
    Ok(Some((span, fonte[span.start..span.end].to_string())))
}

/// `rename`: as edições em todos os arquivos do projeto.
pub(crate) fn renomear(
    projeto: &Projeto,
    uri: &str,
    offset: usize,
    novo: &str,
) -> Result<Vec<Edicao>, String> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Err("O arquivo não pertence a um projeto carregável.".into());
    };
    let Some((alvo, span)) = projeto.identificar(unidade, offset)? else {
        return Err("Não há elemento renomeável nesta posição.".into());
    };
    let antigo = projeto.programa().unit(unidade).source[span.start..span.end].to_string();
    let de_tipo = matches!(
        alvo,
        Alvo::Topo(Element::Class(_) | Element::Typedef(_) | Element::Extension(_))
    );
    validar(novo, de_tipo)?;
    if let Alvo::Topo(el) = alvo {
        let lib = match el {
            Element::Class(c) => projeto.programa().class(c).library,
            Element::Extension(x) => projeto.programa().extension(x).library,
            Element::Typedef(t) => projeto.programa().typedef(t).library,
            Element::Function(f) => projeto.programa().function(f).library,
            Element::Variable(v) => projeto.programa().variable(v).library,
            Element::Prefix(l, _) => l,
        };
        projeto.recusar_externo(lib, &antigo)?;
    }
    if novo == antigo {
        return Ok(Vec::new());
    }
    projeto.conflitos(&alvo, &antigo, novo)?;
    let mut edicoes = Vec::new();
    for (u, inicio, fim) in projeto.ocorrencias(&alvo)? {
        let Some(uri) = projeto.uri_da_unidade(u) else {
            continue;
        };
        edicoes.push(Edicao {
            uri,
            span: Span {
                start: inicio,
                end: fim,
            },
            texto: novo.to_string(),
        });
    }
    // `ocorrencias` já vem ordenada por (unidade, início) e sem repetições.
    Ok(edicoes)
}
