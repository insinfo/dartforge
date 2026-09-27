//! `textDocument/prepareRename` e `textDocument/rename`.
//!
//! O projeto inteiro é carregado ([`crate::projeto::carregar_projeto`]) e o
//! que o cursor denota, com todas as ocorrências, vem do modelo comum de
//! [`crate::projeto`] (o mesmo de definição, hover e referências). Aqui
//! ficam só as regras do renomear:
//!
//! * validação do nome novo (palavra reservada, identificador embutido como
//!   nome de tipo, forma de identificador);
//! * recusa de elementos declarados fora do projeto (SDK, pacotes), inclusive
//!   membros que sobrescrevem um deles;
//! * conflitos por escopo: nome já declarado no mesmo escopo, uso que passaria
//!   a ser sombreado (por local, parâmetro, membro ou parâmetro de tipo) ou
//!   capturado, nome público que viraria privado com usos em outra biblioteca;
//! * o arquivo da classe renomeada, quando o nome dele segue o da classe
//!   (`minha_classe.dart` para `MinhaClasse`), com as diretivas que o citam.

use crate::projeto::{
    Alvo, Dono, Projeto, arquivos_do_projeto, escopo_do_local, nome_base, palavra,
};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, LibraryId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind};
use dartforge_types::Resolved;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use url::Url;

/// Palavras reservadas do Dart: nunca servem de identificador.
const RESERVADAS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
    "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
    "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with",
];

/// Identificadores embutidos: servem de nome de variável e membro, mas não
/// de nome de tipo (nem de prefixo de import).
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

/// O arquivo que acompanha a classe renomeada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenomearArquivo {
    /// URI atual do arquivo.
    pub de: String,
    /// URI nova (mesmo diretório, nome da classe nova em `snake_case`).
    pub para: String,
    /// As diretivas (`import`, `export`, `part`, `part of`) do projeto que
    /// citam o arquivo, com o nome novo. Os spans são do texto antes do
    /// renomear, como as demais edições.
    pub diretivas: Vec<Edicao>,
}

/// O resultado do `rename`: as edições de texto e, quando cabe, o arquivo
/// da classe. O servidor só aplica o arquivo se o cliente aceita operações
/// de recurso e pediu `renameFilesWithClasses: "always"`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Renomeacao {
    pub edicoes: Vec<Edicao>,
    pub arquivo: Option<RenomearArquivo>,
}

/// Valida o nome novo para o tipo de alvo.
fn validar(novo: &str, de_tipo: bool) -> Result<(), String> {
    let b = novo.as_bytes();
    if novo.is_empty()
        || b[0].is_ascii_digit()
        || !b
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$')
    {
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
    /// Recusa o que é declarado fora do projeto.
    fn recusar_fora(&self, alvo: &Alvo, nome: &str) -> Result<(), String> {
        let lib = match alvo {
            Alvo::Topo(el) => self.biblioteca_do_elemento(*el),
            Alvo::Construtor(f) => self.programa().function(*f).library,
            Alvo::Prefixo { biblioteca, .. } => *biblioteca,
            Alvo::Local { unidade, .. } | Alvo::ParametroDeTipo { unidade, .. } => {
                self.programa().unit(*unidade).library
            }
            // A família recusa por conta própria em `ocorrencias`.
            Alvo::Membro { .. } => return Ok(()),
        };
        if self.do_projeto(lib) {
            return Ok(());
        }
        let uri = &self.programa().library(lib).uri;
        Err(format!(
            "'{nome}' é declarado em {uri}, fora do projeto; só declarações do projeto podem ser renomeadas."
        ))
    }

    /// Locais (e parâmetros) da unidade chamados `nome`: offset da
    /// declaração e escopo.
    fn locais_chamados(&self, u: UnitId, nome: &str) -> Vec<(usize, Span)> {
        let unidade = self.programa().unit(u);
        let corpos = &self.consulta.corpos.units[u.0 as usize];
        let mut v: Vec<(usize, Span)> = corpos
            .tipos_de_locais
            .keys()
            .copied()
            .filter(|d| {
                palavra(&unidade.source, *d)
                    .is_some_and(|s| &unidade.source[s.start..s.end] == nome)
            })
            .map(|d| (d, escopo_do_local(&unidade.ast, d)))
            .collect();
        v.sort_by_key(|(d, _)| *d);
        v
    }

    /// As ocorrências que são uso sem qualificação (identificador solto),
    /// sujeitas a sombra por um local ou membro.
    fn usos_soltos(&self, ocorrencias: &[(UnitId, usize, usize)]) -> Vec<(UnitId, Span)> {
        let mut v = Vec::new();
        for &(u, de, ate) in ocorrencias {
            let ast = &self.programa().unit(u).ast;
            let solto = ast.exprs.iter().any(|e| {
                matches!(&e.kind, ExprKind::Identifier(n) if n.span.start == de && n.span.end == ate)
            });
            if solto {
                v.push((
                    u,
                    Span {
                        start: de,
                        end: ate,
                    },
                ));
            }
        }
        v
    }

    /// A classe cuja declaração contém `offset` na unidade (a mais interna).
    fn classe_em(&self, u: UnitId, offset: usize) -> Option<ClassId> {
        let p = self.programa();
        p.classes
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let d = c.decl?;
                let s = p.unit(d.unit).ast.decl(d.decl).span;
                (d.unit == u && s.start <= offset && offset < s.end)
                    .then_some((s.end - s.start, ClassId(i as u32)))
            })
            .min_by_key(|(t, _)| *t)
            .map(|(_, c)| c)
    }

    /// `c` ou um supertipo tem membro chamado `nome`.
    fn tem_membro(&self, c: ClassId, nome: &str) -> bool {
        let p = self.programa();
        self.supertipos(c).into_iter().chain([c]).any(|x| {
            let cx = p.class(x);
            cx.instance_members
                .keys()
                .chain(cx.static_members.keys())
                .any(|s| nome_base(self.nome(*s)) == nome)
        })
    }

    fn conflitos(&self, alvo: &Alvo, antigo: &str, novo: &str) -> Result<(), String> {
        let p = self.programa();
        let ocorrencias: Vec<(UnitId, usize, usize)> =
            self.ocorrencias(alvo, true)?.into_iter().collect();
        match alvo {
            Alvo::Local {
                unidade,
                declaracao,
            } => self.conflitos_de_local(*unidade, *declaracao, novo, &ocorrencias)?,
            Alvo::Membro {
                dono,
                nome,
                estatico,
            } => {
                let fam = self.familia(*dono, nome, *estatico, true)?;
                for c in &fam.classes {
                    if self.tem_membro(*c, novo) {
                        return Err(format!(
                            "A classe '{}' já tem um membro chamado '{novo}'.",
                            self.nome(p.class(*c).name)
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
                self.sombra_por_local(&ocorrencias, novo)?;
                // Um `novo` solto no corpo das classes da família, hoje de
                // topo, passaria a ser o membro renomeado.
                for u in self.unidades() {
                    let ast = &p.unit(u).ast;
                    let corpos = &self.consulta.corpos.units[u.0 as usize];
                    for (i, e) in ast.exprs.iter().enumerate() {
                        if let ExprKind::Identifier(n) = &e.kind
                            && self.nome(n.sym) == novo
                            && matches!(
                                corpos.get_resolved(ast::ExprId(i as u32)),
                                Some(Resolved::Element(_)) | None
                            )
                            && self
                                .classe_em(u, n.span.start)
                                .is_some_and(|c| fam.classes.contains(&c))
                        {
                            return Err(format!(
                                "O uso de '{novo}' em {} passaria a denotar o membro renomeado.",
                                self.uri_da_unidade(u).unwrap_or_default()
                            ));
                        }
                    }
                }
                self.privacidade(antigo, novo, &ocorrencias)?;
            }
            Alvo::Topo(el) => {
                let lib = self.biblioteca_do_elemento(*el);
                let ja = |l: LibraryId| {
                    p.library(l).declared.keys().any(|s| self.nome(*s) == novo)
                        || p.library(l).prefixes.keys().any(|s| self.nome(*s) == novo)
                };
                if ja(lib) {
                    return Err(format!("A biblioteca já declara '{novo}'."));
                }
                for (u, _, _) in &ocorrencias {
                    let l = p.unit(*u).library;
                    if l != lib && ja(l) {
                        return Err(format!(
                            "'{novo}' já é declarado em {}, que usa '{antigo}'.",
                            p.library(l).uri
                        ));
                    }
                }
                self.sombra_por_local(&ocorrencias, novo)?;
                // Dentro de uma classe com membro `novo`, o uso solto
                // passaria a ser o membro.
                for (u, s) in self.usos_soltos(&ocorrencias) {
                    if let Some(c) = self.classe_em(u, s.start)
                        && self.tem_membro(c, novo)
                    {
                        return Err(format!(
                            "O uso de '{antigo}' em '{}' passaria a denotar o membro '{novo}'.",
                            self.nome(p.class(c).name)
                        ));
                    }
                }
                self.privacidade(antigo, novo, &ocorrencias)?;
            }
            Alvo::Construtor(f) => {
                let Some(c) = p.function(*f).class else {
                    return Ok(());
                };
                let cl = p.class(c);
                if cl.constructors.keys().any(|s| self.nome(*s) == novo) {
                    return Err(format!(
                        "A classe '{}' já tem um construtor chamado '{novo}'.",
                        self.nome(cl.name)
                    ));
                }
                if cl
                    .static_members
                    .keys()
                    .any(|s| nome_base(self.nome(*s)) == novo)
                {
                    return Err(format!(
                        "A classe '{}' já tem um membro estático chamado '{novo}'.",
                        self.nome(cl.name)
                    ));
                }
                self.privacidade(antigo, novo, &ocorrencias)?;
            }
            Alvo::Prefixo { biblioteca, .. } => {
                let l = p.library(*biblioteca);
                if l.prefixes.keys().any(|s| self.nome(*s) == novo)
                    || l.scope.keys().any(|s| self.nome(*s) == novo)
                {
                    return Err(format!(
                        "'{novo}' já é um nome visível na biblioteca; o prefixo colidiria com ele."
                    ));
                }
                self.sombra_por_local(&ocorrencias, novo)?;
            }
            Alvo::ParametroDeTipo {
                unidade,
                declaracao,
            } => {
                let ast = &p.unit(*unidade).ast;
                let escopo = escopo_de_parametro_de_tipo(ast, *declaracao);
                // Mesmo nome na mesma lista, ou tipo `novo` usado no escopo
                // (seria capturado pelo parâmetro renomeado).
                let irmaos = listas_de_parametros_de_tipo(ast)
                    .into_iter()
                    .find(|l| l.iter().any(|t| t.name.span.start == *declaracao))
                    .unwrap_or_default();
                if irmaos.iter().any(|t| self.nome(t.name.sym) == novo) {
                    return Err(format!("Já existe um parâmetro de tipo chamado '{novo}'."));
                }
                for t in &ast.types {
                    if let ast::TypeKind::Named { name, .. } = &t.kind
                        && let [n] = &name[..]
                        && self.nome(n.sym) == novo
                        && escopo.start <= n.span.start
                        && n.span.end <= escopo.end
                    {
                        return Err(format!(
                            "O tipo '{novo}' usado neste escopo passaria a denotar o parâmetro de tipo renomeado."
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Conflitos de um local, pelo escopo léxico (bloco, `for`, `catch`,
    /// função): declaração duplicada no mesmo escopo, referência que ficaria
    /// dentro do escopo de outro `novo` aninhado, e uso de `novo` (ou tipo
    /// `novo`) no escopo que passaria a denotar o local renomeado.
    fn conflitos_de_local(
        &self,
        unidade: UnitId,
        declaracao: usize,
        novo: &str,
        ocorrencias: &[(UnitId, usize, usize)],
    ) -> Result<(), String> {
        let u = self.programa().unit(unidade);
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let escopo = escopo_do_local(&u.ast, declaracao);
        let dentro = |s: Span, o: usize| s.start <= o && o < s.end;
        for (outro, escopo_outro) in self.locais_chamados(unidade, novo) {
            if outro == declaracao {
                continue;
            }
            if escopo_outro == escopo {
                return Err(format!("Já existe um local chamado '{novo}' neste escopo."));
            }
            // Um `novo` declarado dentro do escopo do renomeado sombrearia
            // as referências que ficam no escopo dele.
            if dentro(escopo, outro)
                && ocorrencias.iter().any(|(ou, de, _)| {
                    *ou == unidade && *de != declaracao && dentro(escopo_outro, *de)
                })
            {
                return Err(format!(
                    "Uma referência ao local ficaria dentro do escopo de outro '{novo}', que a sombrearia."
                ));
            }
        }
        for (i, e) in u.ast.exprs.iter().enumerate() {
            if let ExprKind::Identifier(n) = &e.kind
                && self.nome(n.sym) == novo
                && dentro(escopo, n.span.start)
                && corpos
                    .declaracao_local(ast::ExprId(i as u32))
                    .is_none_or(|d| !dentro(escopo, d))
            {
                return Err(format!(
                    "O uso de '{novo}' neste escopo passaria a ser sombreado pelo local renomeado."
                ));
            }
        }
        for t in &u.ast.types {
            if let ast::TypeKind::Named { name, .. } = &t.kind
                && let [n] = &name[..]
                && self.nome(n.sym) == novo
                && dentro(escopo, n.span.start)
            {
                return Err(format!(
                    "O tipo '{novo}' usado neste escopo passaria a ser sombreado pelo local renomeado."
                ));
            }
        }
        Ok(())
    }

    /// Algum uso solto do alvo cai no escopo de um local `novo`.
    fn sombra_por_local(
        &self,
        ocorrencias: &[(UnitId, usize, usize)],
        novo: &str,
    ) -> Result<(), String> {
        for (u, s) in self.usos_soltos(ocorrencias) {
            let dentro = |e: Span| e.start <= s.start && s.start < e.end;
            if self
                .locais_chamados(u, novo)
                .iter()
                .any(|(_, e)| dentro(*e))
            {
                return Err(format!(
                    "Um uso em {} passaria a ser sombreado pelo local '{novo}'.",
                    self.uri_da_unidade(u).unwrap_or_default()
                ));
            }
        }
        Ok(())
    }

    /// Público que vira privado: recusa se há usos em outra biblioteca.
    fn privacidade(
        &self,
        antigo: &str,
        novo: &str,
        ocorrencias: &[(UnitId, usize, usize)],
    ) -> Result<(), String> {
        if antigo.starts_with('_') || !novo.starts_with('_') {
            return Ok(());
        }
        let libs: HashSet<LibraryId> = ocorrencias
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

    /// O arquivo da classe acompanha o nome quando se chama como ela
    /// (`MinhaClasse` em `minha_classe.dart`) e o destino ainda não existe.
    fn arquivo_da_classe(&self, alvo: &Alvo, antigo: &str, novo: &str) -> Option<RenomearArquivo> {
        let Alvo::Topo(Element::Class(c)) = alvo else {
            return None;
        };
        let d = self.programa().class(*c).decl?;
        let caminho = self.programa().unit(d.unit).path.clone()?;
        if caminho.file_stem()?.to_str()? != snake_case(antigo) {
            return None;
        }
        let destino = caminho.with_file_name(format!("{}.dart", snake_case(novo)));
        if destino.exists() || destino == caminho {
            return None;
        }
        let nome_novo = destino.file_name()?.to_str()?.to_string();
        let mut diretivas = Vec::new();
        for arquivo in arquivos_do_projeto(&self.raiz) {
            let Some(u) = Url::from_file_path(&arquivo)
                .ok()
                .and_then(|x| self.unidade_do_uri(x.as_str()))
            else {
                continue;
            };
            let unidade = self.programa().unit(u);
            for dir in &unidade.unit.directives {
                let lit = match &dir.kind {
                    ast::DirectiveKind::Import { uri, .. }
                    | ast::DirectiveKind::Export { uri, .. }
                    | ast::DirectiveKind::Part { uri } => uri,
                    ast::DirectiveKind::PartOf { uri: Some(uri), .. } => uri,
                    _ => continue,
                };
                let Some(valor) = dartforge_elements::load::string_lit_value(lit) else {
                    continue;
                };
                if resolver_uri(&arquivo, &valor).is_none_or(|alvo| {
                    dartforge_elements::gerado::chave(&alvo)
                        != dartforge_elements::gerado::chave(&caminho)
                }) {
                    continue;
                }
                // O nome do arquivo é o fim do texto do literal (sem escapes).
                let antigo_nome = caminho.file_name()?.to_str()?;
                let texto_lit = &unidade.source[lit.span.start..lit.span.end];
                let Some(pos) = texto_lit.rfind(antigo_nome) else {
                    continue;
                };
                let inicio = lit.span.start + pos;
                diretivas.push(Edicao {
                    uri: Url::from_file_path(&arquivo).ok()?.to_string(),
                    span: Span {
                        start: inicio,
                        end: inicio + antigo_nome.len(),
                    },
                    texto: nome_novo.clone(),
                });
            }
        }
        Some(RenomearArquivo {
            de: Url::from_file_path(&caminho).ok()?.to_string(),
            para: Url::from_file_path(&destino).ok()?.to_string(),
            diretivas,
        })
    }
}

/// `MinhaClasse` → `minha_classe` (a convenção de nomes de arquivo do Dart).
pub(crate) fn snake_case(nome: &str) -> String {
    let mut saida = String::new();
    let chars: Vec<char> = nome.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let anterior_minusculo =
                i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let proximo_minusculo = chars.get(i + 1).is_some_and(|p| p.is_lowercase());
            if i > 0 && (anterior_minusculo || (proximo_minusculo && chars[i - 1].is_uppercase())) {
                saida.push('_');
            }
            saida.extend(c.to_lowercase());
        } else {
            saida.push(*c);
        }
    }
    saida
}

/// O arquivo que a URI de uma diretiva escrita em `arquivo` aponta
/// (relativa, `file:` ou `package:` pelo `package_config.json`).
fn resolver_uri(arquivo: &Path, uri: &str) -> Option<PathBuf> {
    if uri.starts_with("dart:") {
        return None;
    }
    if uri.starts_with("package:") {
        let config = dartforge_elements::config::PackageConfig::discover(arquivo)?;
        return dartforge_elements::config::PackageConfig::load(&config)
            .ok()?
            .resolve_package_uri(uri)
            .ok();
    }
    if uri.starts_with("file:") {
        return Url::parse(uri).ok()?.to_file_path().ok();
    }
    let destino = arquivo.parent()?.join(uri);
    Some(dartforge_elements::config::sem_verbatim(
        std::fs::canonicalize(&destino).unwrap_or(destino),
    ))
}

/// As listas de parâmetros de tipo da unidade.
fn listas_de_parametros_de_tipo(ast: &ast::Ast) -> Vec<Vec<&ast::TypeParameter>> {
    let mut v: Vec<Vec<&ast::TypeParameter>> = Vec::new();
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
        v.push(ps.iter().collect());
    }
    for f in &ast.functions {
        v.push(f.type_params.iter().collect());
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind {
            v.push(type_params.iter().collect());
        }
    }
    v
}

/// O trecho em que o parâmetro de tipo declarado em `declaracao` vale: a
/// declaração, função ou tipo de função que o declara.
fn escopo_de_parametro_de_tipo(ast: &ast::Ast, declaracao: usize) -> Span {
    let declara = |ps: &[ast::TypeParameter]| ps.iter().any(|t| t.name.span.start == declaracao);
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
        if declara(ps) {
            return d.span;
        }
    }
    for f in &ast.functions {
        if declara(&f.type_params) {
            return f.span;
        }
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind
            && declara(type_params)
        {
            return t.span;
        }
    }
    Span {
        start: declaracao,
        end: declaracao,
    }
}

/// `prepareRename`: o intervalo do nome e o texto atual, `Ok(None)` quando
/// não há nome renomeável sob o cursor.
///
/// # Erros
///
/// O elemento existe mas não pode ser renomeado (fora do projeto, sem
/// resolução).
pub(crate) fn preparar(
    projeto: &Projeto,
    uri: &str,
    offset: usize,
) -> Result<Option<(Span, String)>, String> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Ok(None);
    };
    let Some(d) = projeto.identificar(unidade, offset)? else {
        return Ok(None);
    };
    let fonte = &projeto.programa().unit(unidade).source;
    let texto = fonte[d.nome.start..d.nome.end].to_string();
    // Recusa cedo o que está fora do projeto.
    projeto.recusar_fora(&d.alvo, &texto)?;
    projeto.ocorrencias(&d.alvo, true)?;
    Ok(Some((d.nome, texto)))
}

/// `rename`: as edições em todos os arquivos do projeto.
///
/// # Erros
///
/// Nome inválido, elemento não renomeável ou conflito, com a mensagem para
/// o usuário.
pub(crate) fn renomear(
    projeto: &Projeto,
    uri: &str,
    offset: usize,
    novo: &str,
) -> Result<Renomeacao, String> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Err("O arquivo não pertence a um projeto carregável.".into());
    };
    let Some(d) = projeto.identificar(unidade, offset)? else {
        return Err("Não há elemento renomeável nesta posição.".into());
    };
    let antigo = projeto.programa().unit(unidade).source[d.nome.start..d.nome.end].to_string();
    let de_tipo = matches!(
        d.alvo,
        Alvo::Topo(Element::Class(_) | Element::Typedef(_) | Element::Extension(_))
            | Alvo::ParametroDeTipo { .. }
            | Alvo::Prefixo { .. }
    );
    validar(novo, de_tipo)?;
    projeto.recusar_fora(&d.alvo, &antigo)?;
    if novo == antigo {
        return Ok(Renomeacao::default());
    }
    projeto.conflitos(&d.alvo, &antigo, novo)?;
    let mut edicoes = Vec::new();
    for (u, inicio, fim) in projeto.ocorrencias(&d.alvo, true)? {
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
    Ok(Renomeacao {
        edicoes,
        arquivo: projeto.arquivo_da_classe(&d.alvo, &antigo, novo),
    })
}

#[cfg(test)]
mod testes {
    use super::snake_case;

    #[test]
    fn nomes_de_arquivo() {
        assert_eq!(snake_case("MinhaClasse"), "minha_classe");
        assert_eq!(snake_case("HTTPServer"), "http_server");
        assert_eq!(snake_case("A"), "a");
        assert_eq!(snake_case("Base64Codec"), "base64_codec");
    }
}
