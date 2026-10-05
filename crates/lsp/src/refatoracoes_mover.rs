//! Move top-level to file (`AS:src/services/refactoring/move_top_level_to_file.dart`,
//! docs/LSP-ESPECIFICACAO.md §13.11.9 e): o `compute`, com o
//! `ImportAnalyzer` (`AS:src/utilities/import_analyzer.dart`), o cabeçalho do
//! arquivo, as faixas dos grupos e o `importLibrary`/`_addLibraryImports` do
//! `DartFileEditBuilderImpl` (`analyzer_plugin`, `change_builder_dart.dart`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::refatoracoes::{Contexto, ResultadoDeRefatoracao};
use crate::refatoracoes_exec::{Mudanca, Texto, regra_ligada};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, LibraryId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind};
use dartforge_frontend::token::Kind;
use dartforge_types::{MemberRef, Resolved};
use std::path::{Path, PathBuf};

type R = ResultadoDeRefatoracao;

/// As linhas de um texto (`LineInfo`): `\r\n`, `\r` e `\n` terminam linha.
struct Linhas {
    inicios: Vec<usize>,
}

impl Linhas {
    fn de(t: &str) -> Linhas {
        let b = t.as_bytes();
        let mut inicios = vec![0];
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\r' => {
                    if i + 1 < b.len() && b[i + 1] == b'\n' {
                        i += 1;
                    }
                    inicios.push(i + 1);
                }
                b'\n' => inicios.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        Linhas { inicios }
    }

    /// `getLocation(offset).lineNumber` (a partir de 1).
    fn numero(&self, offset: usize) -> usize {
        self.inicios.partition_point(|&s| s <= offset)
    }

    /// `getOffsetOfLine(indice)` (a partir de 0).
    fn inicio(&self, indice: usize) -> usize {
        self.inicios.get(indice).copied().unwrap_or_else(|| *self.inicios.last().unwrap())
    }
}

/// Uma diretiva do arquivo (lida do texto).
#[derive(Debug, Clone)]
struct Diretiva {
    especie: &'static str,
    /// O texto da URI (sem as aspas).
    uri: String,
    prefixo: String,
    /// Com o comentário de documentação e as anotações.
    inicio: usize,
    inicio_sem_meta: usize,
    fim: usize,
    mostrados: Vec<(Span, Vec<String>)>,
    ocultos: Vec<(Span, Vec<String>)>,
}

/// Uma unidade preparada para os imports (`DartFileEditBuilderImpl`).
struct Construtor {
    uri: String,
    caminho: Option<PathBuf>,
    /// O texto (vazio num arquivo novo).
    fonte: String,
    /// A unidade, quando existe no programa (as opções de lint).
    unidade: Option<UnitId>,
    diretivas: Vec<Diretiva>,
    /// O início da primeira declaração.
    primeira_declaracao: Option<usize>,
    /// `librariesToImport`, na ordem de inserção.
    importar: Vec<ImportPendente>,
    cabecalho: Option<String>,
}

/// `_LibraryImport`.
#[derive(Debug, Clone)]
struct ImportPendente {
    chave: String,
    texto: String,
    prefixos: Vec<String>,
    mostrados: Vec<Vec<String>>,
    ocultos: Vec<Vec<String>>,
}

impl ImportPendente {
    fn todos_mostrados(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for l in &self.mostrados {
            for n in l {
                if !v.contains(n) {
                    v.push(n.clone());
                }
            }
        }
        v
    }

    fn todos_ocultos(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for l in &self.ocultos {
            for n in l {
                if !v.contains(n) {
                    v.push(n.clone());
                }
            }
        }
        v
    }

    /// `ensureShown`.
    fn garantir_mostrado(&mut self, nome: &str, usar_show: bool) {
        if self.mostrados.is_empty() && usar_show {
            self.mostrados.push(vec![nome.to_string()]);
        } else if let Some(l) = self.mostrados.last_mut() {
            l.push(nome.to_string());
        }
        for l in self.ocultos.iter_mut() {
            l.retain(|n| n != nome);
        }
        self.ocultos.retain(|l| !l.is_empty());
    }
}

/// `DirectiveSortPriority` de um import.
fn prioridade(uri: &str) -> u8 {
    if uri.starts_with("dart:") {
        0
    } else if uri.starts_with("package:") {
        1
    } else if uri.contains("://") {
        2
    } else {
        3
    }
}

/// `compareDirectiveUri`.
fn comparar_uris(a: &str, b: &str) -> std::cmp::Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:")) && !a.starts_with('/') && !b.starts_with('/') {
        return a.cmp(b);
    }
    let (Some(ia), Some(ib)) = (a.find('/'), b.find('/')) else { return a.cmp(b) };
    a[..ia].cmp(&b[..ib]).then_with(|| a[ia + 1..].cmp(&b[ib + 1..]))
}

/// O `pathToUri` do conversor da sessão: `package:` para um arquivo sob o
/// `lib/` de um pacote, senão `file:`.
fn uri_do_caminho(caminho: &Path) -> String {
    let configuracao = dartforge_elements::config::PackageConfig::discover(caminho).and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
    let canonico = match (caminho.parent().and_then(|p| p.canonicalize().ok()), caminho.file_name()) {
        (Some(pasta), Some(nome)) => dartforge_elements::config::sem_verbatim(pasta.join(nome)),
        _ => caminho.to_path_buf(),
    };
    if let Some(c) = &configuracao {
        for (nome, pasta) in &c.package_dirs {
            if let Ok(rel) = canonico.strip_prefix(pasta) {
                return format!("package:{nome}/{}", rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    url::Url::from_file_path(caminho).map(|u| u.to_string()).unwrap_or_default()
}

impl Construtor {
    fn novo(uri: String, caminho: Option<PathBuf>, fonte: String, unidade: Option<UnitId>, cabecalho: Option<String>) -> Construtor {
        let mut nomes = dartforge_intern::Interner::new();
        let analisado = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        let mut diretivas = Vec::new();
        for d in analisado.unit.directives.iter() {
            let inicio = inicio_com_meta(&fonte, d.span.start, &d.metadata);
            let mut x = Diretiva {
                especie: "",
                uri: String::new(),
                prefixo: String::new(),
                inicio,
                inicio_sem_meta: d.span.start,
                fim: d.span.end,
                mostrados: Vec::new(),
                ocultos: Vec::new(),
            };
            match &d.kind {
                ast::DirectiveKind::Library { .. } => x.especie = "library",
                ast::DirectiveKind::Import { uri, prefix, combinators, .. } => {
                    x.especie = "import";
                    x.uri = fonte[uri.span.start..uri.span.end].trim_matches(|c| c == '\'' || c == '"').to_string();
                    x.prefixo = prefix.map(|p| fonte[p.span.start..p.span.end].to_string()).unwrap_or_default();
                    for c in combinators.iter() {
                        match c {
                            ast::Combinator::Show(ns) => {
                                x.mostrados.push((combinador_span(&fonte, ns, "show"), ns.iter().map(|n| fonte[n.span.start..n.span.end].to_string()).collect()))
                            }
                            ast::Combinator::Hide(ns) => {
                                x.ocultos.push((combinador_span(&fonte, ns, "hide"), ns.iter().map(|n| fonte[n.span.start..n.span.end].to_string()).collect()))
                            }
                        }
                    }
                }
                ast::DirectiveKind::Export { .. } => x.especie = "export",
                ast::DirectiveKind::Part { .. } => x.especie = "part",
                _ => x.especie = "outra",
            }
            diretivas.push(x);
        }
        let primeira_declaracao = analisado.unit.declarations.first().map(|&d| {
            let decl = analisado.ast.decl(d);
            inicio_com_meta(&fonte, decl.span.start, &decl.metadata)
        });
        Construtor { uri, caminho, fonte, unidade, diretivas, primeira_declaracao, importar: Vec::new(), cabecalho }
    }

    /// A URI canônica de uma URI escrita num import deste arquivo.
    fn uri_canonica(&self, escrita: &str) -> String {
        if escrita.contains(':') {
            return escrita.to_string();
        }
        match self.caminho.as_deref().and_then(|c| c.parent()) {
            Some(pasta) => uri_do_caminho(&pasta.join(escrita)),
            None => escrita.to_string(),
        }
    }

    /// `_getLibraryUriText`.
    fn texto_da_uri(&self, cx: &Contexto<'_>, uri: &str) -> String {
        let pasta = self.caminho.as_deref().and_then(|c| c.parent()).map(Path::to_path_buf);
        if let Ok(u) = url::Url::parse(uri)
            && u.scheme() == "file"
            && let (Ok(alvo), Some(pasta)) = (u.to_file_path(), &pasta)
        {
            return crate::refatoracoes_metodo::caminho_relativo(&alvo, pasta);
        }
        let relativo = self.unidade.is_some_and(|u| regra_ligada(cx.p, u, "prefer_relative_imports"));
        if relativo {
            let proprio = uri_do_caminho(self.caminho.as_deref().unwrap_or(Path::new("")));
            let pacote = |s: &str| s.strip_prefix("package:").and_then(|r| r.split('/').next()).map(str::to_string);
            if let (Some(a), Some(b)) = (pacote(uri), pacote(&proprio))
                && a == b
                && let Some(pasta) = &pasta
            {
                // `uriToPath`: pelo `package_config`.
                let configuracao = self
                    .caminho
                    .as_deref()
                    .and_then(dartforge_elements::config::PackageConfig::discover)
                    .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
                if let Some(c) = configuracao
                    && let Ok(alvo) = c.resolve_package_uri(uri)
                {
                    return crate::refatoracoes_metodo::caminho_relativo(&alvo, pasta);
                }
            }
        }
        uri.to_string()
    }

    /// `_importLibrary(uri, prefix, shownName, useShow)`.
    fn importar_biblioteca(&mut self, cx: &Contexto<'_>, uri: &str, prefixo: Option<&str>, mostrar: Option<&str>, usar_show: bool) {
        if let Some(i) = self.importar.iter_mut().find(|i| i.chave == uri) {
            if let Some(p) = prefixo
                && !i.prefixos.iter().any(|x| x == p)
            {
                i.prefixos.push(p.to_string());
            }
            if let Some(n) = mostrar {
                i.garantir_mostrado(n, usar_show);
            }
            return;
        }
        let texto = self.texto_da_uri(cx, uri);
        // Os `show`/`hide` dos imports existentes com a mesma URI e prefixo.
        let (mut mostrados, mut ocultos): (Vec<Vec<String>>, Vec<Vec<String>>) = (Vec::new(), Vec::new());
        for d in self.diretivas.iter().filter(|d| d.especie == "import") {
            if d.prefixo != prefixo.unwrap_or("") {
                continue;
            }
            if self.texto_da_uri(cx, &self.uri_canonica(&d.uri)) != texto {
                continue;
            }
            mostrados.extend(d.mostrados.iter().map(|(_, n)| n.clone()));
            ocultos.extend(d.ocultos.iter().map(|(_, n)| n.clone()));
        }
        let mut i = ImportPendente {
            chave: uri.to_string(),
            texto,
            prefixos: vec![prefixo.unwrap_or("").to_string()],
            mostrados,
            ocultos,
        };
        if let Some(n) = mostrar {
            i.garantir_mostrado(n, usar_show);
        }
        self.importar.push(i);
    }

    /// `finalize`: os imports (`_addLibraryImports`) e o cabeçalho.
    fn finalizar(&self, cx: &Contexto<'_>, m: &mut Mudanca, eol: &str) {
        if !self.importar.is_empty() {
            self.adicionar_imports(cx, m, eol);
        }
        if let Some(c) = &self.cabecalho {
            // `addInsertion(0, insertBeforeExisting: true)`.
            inserir_antes(m, &self.uri, 0, format!("{c}{eol}"));
        }
    }

    /// `_addLibraryImports`.
    fn adicionar_imports(&self, cx: &Contexto<'_>, m: &mut Mudanca, eol: &str) {
        let fonte = self.fonte.as_str();
        let biblioteca: Option<usize> = self.diretivas.iter().find(|d| d.especie == "library").map(|d| d.fim);
        let imports: Vec<&Diretiva> = self.diretivas.iter().filter(|d| d.especie == "import").collect();
        let primeiro_export: Option<usize> = self.diretivas.iter().find(|d| d.especie == "export").map(|d| d.inicio);
        let primeira_parte: Option<usize> = self.diretivas.iter().find(|d| d.especie == "part").map(|d| d.inicio);
        let primeira_diretiva: Option<usize> = self.diretivas.first().map(|d| d.inicio);
        let primeira_declaracao = self.primeira_declaracao;
        let fim_da_unidade = fonte.len();
        let mut lista = self.importar.clone();
        lista.sort_by(|a, b| {
            let (pa, pb) = (prioridade(&a.texto), prioridade(&b.texto));
            if pa == pb { comparar_uris(&a.texto, &b.texto) } else { pa.cmp(&pb) }
        });
        let ordenar = self.unidade.is_some_and(|u| regra_ligada(cx.p, u, "combinators_ordering"));
        // As aspas preferidas, pelos imports.
        let aspas = match self.unidade {
            Some(u) if regra_ligada(cx.p, u, "prefer_single_quotes") => '\'',
            Some(u) if regra_ligada(cx.p, u, "prefer_double_quotes") => '"',
            _ => {
                let duplas = imports.iter().filter(|d| fonte[d.inicio_sem_meta..d.fim].contains("\"")).count();
                let simples = imports.len() - duplas;
                if duplas > simples { '"' } else { '\'' }
            }
        };
        let escrever = |i: &ImportPendente| -> String {
            let mut s = String::new();
            let mut prefixos = i.prefixos.clone();
            prefixos.sort();
            for (k, p) in prefixos.iter().enumerate() {
                if k > 0 {
                    s.push_str(eol);
                }
                s.push_str(&format!("import {aspas}{}{aspas}", i.texto));
                if !p.is_empty() {
                    s.push_str(" as ");
                    s.push_str(p);
                }
                let mostrados = i.todos_mostrados();
                if !mostrados.is_empty() {
                    let mut v = mostrados.clone();
                    if ordenar {
                        v.sort();
                    }
                    s.push_str(&format!(" show {}", v.join(", ")));
                }
                let ocultos = i.todos_ocultos();
                if !ocultos.is_empty() {
                    let mut v = ocultos.clone();
                    if ordenar {
                        v.sort();
                    }
                    s.push_str(&format!(" hide {}", v.join(", ")));
                }
                s.push(';');
            }
            s
        };
        if !imports.is_empty() {
            for i in &lista {
                let dart = i.texto.starts_with("dart:");
                let pacote = i.texto.starts_with("package:");
                let mut inserido = false;
                let mut ultimo: Option<usize> = None;
                let mut ultimo_dart: Option<usize> = None;
                let mut ultimo_pacote: Option<usize> = None;
                let (mut ultimo_e_dart, mut ultimo_e_pacote) = (false, false);
                for (k, e) in imports.iter().enumerate() {
                    let e_dart = e.uri.starts_with("dart:");
                    let e_pacote = e.uri.starts_with("package:");
                    let e_relativo = !e.uri.contains(':');
                    let substitui = i.texto == e.uri && i.prefixos.first().map(String::as_str).unwrap_or("") == e.prefixo;
                    let antes = i.texto.as_str() < e.uri.as_str();
                    // `insert(prev, replace, next, trailingNewLine)`.
                    let mut inserir = |anterior: Option<usize>, proximo: usize, linha_depois: bool, m: &mut Mudanca| {
                        if let Some(a) = anterior {
                            let d = &imports[a];
                            let mut offset = d.fim;
                            // Comentários na mesma linha depois do anterior.
                            let linhas = Linhas::de(fonte);
                            let linha = linhas.numero(offset);
                            for c in dartforge_frontend::comentarios::Comentarios::de(fonte).todos() {
                                if c.start >= d.fim && linhas.numero(c.start) == linha && fonte[d.fim..c.start].trim().is_empty() {
                                    offset = c.end;
                                }
                            }
                            m.adicionar(&self.uri, Span { start: offset, end: offset }, format!("{eol}{}", escrever(i)));
                        } else {
                            let d = &imports[proximo];
                            let offset = if Some(d.inicio) == primeira_diretiva { d.inicio_sem_meta } else { d.inicio };
                            let mut t = format!("{}{eol}", escrever(i));
                            if linha_depois {
                                t.push_str(eol);
                            }
                            m.adicionar(&self.uri, Span { start: offset, end: offset }, t);
                        }
                    };
                    if substitui {
                        self.atualizar_combinadores(m, i, &e.mostrados, &e.ocultos, ordenar);
                        inserido = true;
                        break;
                    } else if dart {
                        if !e_dart || antes {
                            inserir(ultimo_dart, k, !e_dart, m);
                            inserido = true;
                            break;
                        }
                    } else if pacote {
                        if e_relativo || antes {
                            inserir(ultimo_pacote, k, e_relativo, m);
                            inserido = true;
                            break;
                        }
                    } else if !e_dart && !e_pacote && antes {
                        inserir(None, k, false, m);
                        inserido = true;
                        break;
                    }
                    ultimo = Some(k);
                    if e_dart {
                        ultimo_dart = Some(k);
                    } else if e_pacote {
                        ultimo_pacote = Some(k);
                    }
                    ultimo_e_dart = e_dart;
                    ultimo_e_pacote = e_pacote;
                }
                if !inserido && let Some(u) = ultimo {
                    let mut t = String::new();
                    if pacote {
                        if ultimo_e_dart {
                            t.push_str(eol);
                        }
                    } else if !dart && (ultimo_e_dart || ultimo_e_pacote) {
                        t.push_str(eol);
                    }
                    t.push_str(eol);
                    t.push_str(&escrever(i));
                    let fim = imports[u].fim;
                    m.adicionar(&self.uri, Span { start: fim, end: fim }, t);
                }
            }
            return;
        }
        if let Some(fim) = biblioteca {
            let mut t = format!("{eol}{eol}");
            for (k, i) in lista.iter().enumerate() {
                t.push_str(&escrever(i));
                if k != lista.len() - 1 {
                    t.push_str(eol);
                }
            }
            m.adicionar(&self.uri, Span { start: fim, end: fim }, t);
            return;
        }
        if let Some(o) = primeiro_export.or(primeira_parte) {
            let mut t = String::new();
            for i in &lista {
                t.push_str(&escrever(i));
                t.push_str(eol);
            }
            t.push_str(eol);
            m.adicionar(&self.uri, Span { start: o, end: o }, t);
            return;
        }
        // Antes da primeira declaração, ou junto das outras edições.
        let edicoes_do_arquivo: Vec<usize> = m.arquivos.iter().find(|(u, _)| *u == self.uri).map(|(_, l)| l.iter().map(|e| e.span.start).collect()).unwrap_or_default();
        let (offset, linha_depois) = if let Some(o) = primeira_declaracao {
            (o, true)
        } else if let Some(&o) = edicoes_do_arquivo.iter().min() {
            (o, true)
        } else {
            (fim_da_unidade, false)
        };
        let mut t = String::new();
        for (k, i) in lista.iter().enumerate() {
            t.push_str(&escrever(i));
            t.push_str(eol);
            if k == lista.len() - 1 && linha_depois {
                t.push_str(eol);
            }
        }
        inserir_antes(m, &self.uri, offset, t);
    }

    /// `updateHideCombinators` e `updateShowCombinators` de um import que já
    /// existe.
    fn atualizar_combinadores(&self, m: &mut Mudanca, i: &ImportPendente, mostrados: &[(Span, Vec<String>)], ocultos: &[(Span, Vec<String>)], ordenar: bool) {
        let fonte = self.fonte.as_str();
        let todos_ocultos = i.todos_ocultos();
        let todos_mostrados = i.todos_mostrados();
        for (s, nomes) in ocultos {
            let novos: Vec<String> = nomes.iter().filter(|n| todos_ocultos.contains(n) && !todos_mostrados.contains(n)).cloned().collect();
            if novos.is_empty() {
                // Do fim do token anterior ao fim do `hide`.
                let anterior = fonte[..s.start].trim_end().len();
                m.adicionar(&self.uri, Span { start: anterior, end: s.end }, "");
            } else if novos.len() != nomes.len() {
                let mut v = novos.clone();
                if ordenar {
                    v.sort();
                }
                m.adicionar(&self.uri, *s, format!("hide {}", v.join(", ")));
            }
        }
        if i.mostrados.is_empty() || mostrados.is_empty() {
            return;
        }
        let existentes: Vec<String> = mostrados.iter().flat_map(|(_, n)| n.iter().cloned()).collect();
        let novos: Vec<String> = todos_mostrados.iter().filter(|n| !existentes.contains(n)).cloned().collect();
        if novos.is_empty() {
            return;
        }
        let ordenados = existentes.windows(2).all(|w| w[0] <= w[1]);
        let (ultimo_span, ultimos) = mostrados.last().unwrap();
        if ordenar || ordenados {
            let mut v: Vec<String> = ultimos.clone();
            for n in &novos {
                if !v.contains(n) {
                    v.push(n.clone());
                }
            }
            v.sort();
            m.adicionar(&self.uri, *ultimo_span, format!("show {}", v.join(", ")));
        } else {
            let mut v = novos.clone();
            v.sort();
            m.adicionar(&self.uri, Span { start: ultimo_span.end, end: ultimo_span.end }, format!(", {}", v.join(", ")));
        }
    }
}

/// `addInsertion(offset, insertBeforeExisting: true)`: a inserção fica antes
/// das outras do mesmo offset no texto.
fn inserir_antes(m: &mut Mudanca, uri: &str, offset: usize, texto: String) {
    // Na ordem do `SourceFileEdit` (decrescente), a inserção entra depois de
    // todas as de offset maior ou igual: no texto, antes delas.
    let lista = match m.arquivos.iter().position(|(u, _)| u == uri) {
        Some(i) => &mut m.arquivos[i].1,
        None => {
            m.arquivos.push((uri.to_string(), Vec::new()));
            &mut m.arquivos.last_mut().unwrap().1
        }
    };
    let mut i = 0;
    while i < lista.len() && lista[i].span.start >= offset {
        i += 1;
    }
    lista.insert(i, crate::Edicao { uri: uri.to_string(), span: Span { start: offset, end: offset }, texto });
}

/// O início de uma diretiva ou declaração com o comentário de documentação
/// e as anotações.
fn inicio_com_meta(fonte: &str, inicio: usize, meta: &[ast::Annotation]) -> usize {
    let mut ini = meta.iter().map(|a| a.span.start).min().unwrap_or(inicio).min(inicio);
    if let Some(d) = dartforge_frontend::comentarios::Comentarios::de(fonte).dart_doc(fonte, ini) {
        ini = ini.min(d.start);
    }
    ini
}

/// O intervalo de um combinador `show`/`hide` (da palavra ao último nome).
fn combinador_span(fonte: &str, nomes: &[ast::Name], palavra: &str) -> Span {
    let primeiro = nomes.first().map_or(0, |n| n.span.start);
    let ultimo = nomes.last().map_or(primeiro, |n| n.span.end);
    let antes = fonte[..primeiro].trim_end();
    let inicio = if antes.ends_with(palavra) { antes.len() - palavra.len() } else { primeiro };
    Span { start: inicio, end: ultimo }
}

/// `fileHeader`: o primeiro comentário antes do primeiro token (depois do
/// `#!`): um `/* */` não doc, ou as linhas `//` consecutivas.
fn cabecalho_do_arquivo(fonte: &str) -> Option<(usize, usize)> {
    let tokens = dartforge_frontend::lexer::lex(fonte).ok()?;
    let mut primeiro = tokens.first()?;
    if primeiro.kind == Kind::ScriptTag {
        primeiro = tokens.get(1)?;
    }
    let inicio_da_busca = if tokens[0].kind == Kind::ScriptTag { tokens[0].span.end } else { 0 };
    let comentarios: Vec<Span> = dartforge_frontend::comentarios::Comentarios::de(fonte)
        .todos()
        .iter()
        .copied()
        .filter(|c| c.start >= inicio_da_busca && c.end <= primeiro.span.start)
        .collect();
    let c0 = *comentarios.first()?;
    let lexema = &fonte[c0.start..c0.end];
    if lexema.starts_with("/**") || lexema.starts_with("///") {
        return None;
    }
    if lexema.starts_with("/*") {
        return Some((c0.start, c0.end));
    }
    if !lexema.starts_with("//") {
        return None;
    }
    let linhas = Linhas::de(fonte);
    let mut fim = c0.end;
    let mut linha_anterior = linhas.numero(c0.start);
    for c in comentarios.iter().skip(1) {
        let l = &fonte[c.start..c.end];
        if !l.starts_with("//") || l.starts_with("///") {
            break;
        }
        let linha = linhas.numero(c.start);
        if linha != linha_anterior + 1 {
            break;
        }
        fim = c.end;
        linha_anterior = linha;
    }
    Some((c0.start, fim))
}

/// O `ImportAnalyzer`.
#[derive(Default)]
struct AnalisadorDeImports {
    movendo: Vec<Element>,
    ficando: Vec<Element>,
    /// Elemento → os índices (em `imports` da biblioteca) usados.
    refs_movendo: Vec<(Element, Vec<usize>)>,
    refs_ficando: Vec<(Element, Vec<usize>)>,
}

impl AnalisadorDeImports {
    fn registrar(lista: &mut Vec<(Element, Vec<usize>)>, el: Element, import: Option<usize>) {
        let i = match lista.iter().position(|(e, _)| *e == el) {
            Some(i) => i,
            None => {
                lista.push((el, Vec::new()));
                lista.len() - 1
            }
        };
        if let Some(k) = import
            && !lista[i].1.contains(&k)
        {
            lista[i].1.push(k);
        }
    }

    fn tem_movendo_para_ficando(&self) -> bool {
        self.ficando.iter().any(|d| self.refs_movendo.iter().any(|(e, _)| e == d))
    }

    fn tem_ficando_para_movendo(&self) -> bool {
        self.movendo.iter().any(|d| self.refs_ficando.iter().any(|(e, _)| e == d))
    }
}

/// `dart.refactor.move_top_level_to_file` (`compute`).
pub(crate) fn executar(cx: &Contexto<'_>, uri: &str, offset: usize, comprimento: usize, destino: &str) -> ResultadoDeRefatoracao {
    let p = cx.p;
    let prog = p.programa();
    let Some(membros) = cx.membros_a_mover(offset, comprimento) else { return R::Falha(None) };
    let Ok(url_destino) = url::Url::parse(destino) else {
        return R::ErroInterno(format!("FormatException: Invalid URI: {destino}"));
    };
    let Ok(caminho_destino) = url_destino.to_file_path() else {
        return R::ErroInterno(format!("Unsupported operation: Cannot extract a file path from a {} URI", url_destino.scheme()));
    };
    let uri_destino = url::Url::from_file_path(&caminho_destino).map(|u| u.to_string()).unwrap_or_else(|_| destino.to_string());
    let import_destino = uri_do_caminho(&caminho_destino);
    let tx = Texto::novo(cx.fonte);
    let eol = tx.eol();
    let linhas = Linhas::de(cx.fonte);
    // O destino existe (no disco ou aberto)?
    let unidade_destino = p.unidade_do_uri(&uri_destino);
    let texto_destino: Option<String> = match unidade_destino {
        Some(u) => Some(prog.unit(u).source.clone()),
        None => std::fs::read_to_string(&caminho_destino).ok(),
    };
    let existe = texto_destino.is_some();
    let mut cabecalho: Option<String> = None;
    let mut posicao = 0usize;
    let mut linha_antes = false;
    if !existe {
        if let Some((a, b)) = cabecalho_do_arquivo(cx.fonte) {
            cabecalho = Some(cx.fonte[a..b].to_string());
        }
    } else {
        posicao = texto_destino.as_ref().map_or(0, |t| t.len());
        linha_antes = true;
    }
    // A faixa do grupo.
    let nos: Vec<usize> = membros
        .iter()
        .filter_map(|(d, _)| cx.arvore.nos.iter().position(|k| k.marca == crate::arvore_analyzer::Marca::Decl(*d) || funcao_de_topo(cx, k.marca, *d)))
        .collect();
    let faixa = |com_linha_anterior: bool| -> (usize, usize) {
        let primeiro = nos.first().copied().unwrap_or(0);
        let ultimo = nos.last().copied().unwrap_or(0);
        let mut inicio = cx.arvore.nos[primeiro].inicio;
        if com_linha_anterior {
            let linha_inicial = linhas.numero(inicio);
            // `beginToken.previous`: nulo quando o membro começa num
            // comentário.
            let comeca_em_comentario = cx.comentarios.iter().any(|c| c.start == inicio);
            if !comeca_em_comentario {
                let anterior = cx.tokens.iter().rev().find(|t| t.span.end <= inicio && t.kind != Kind::Eof);
                if let Some(t) = anterior {
                    let linha_anterior = linhas.numero(t.span.start);
                    if linha_anterior + 1 < linha_inicial {
                        inicio = linhas.inicio(linha_anterior);
                    }
                }
            }
        }
        let mut fim = cx.arvore.nos[ultimo].fim;
        let linha_final = linhas.numero(fim);
        match cx.tokens.iter().find(|t| t.span.start >= fim && t.kind != Kind::Eof) {
            None => fim = cx.fonte.len(),
            Some(t) => {
                let proxima = linhas.numero(t.span.start);
                if linha_final + 1 < proxima {
                    fim = linhas.inicio(linha_final);
                }
            }
        }
        (inicio, fim)
    };
    let faixa_sem = faixa(false);
    let faixa_com = faixa(true);
    // O `ImportAnalyzer`.
    let analisador = analisar_imports(cx, faixa_sem);
    let lib = prog.unit(cx.unidade).library;
    let imports_da_lib: Vec<&dartforge_elements::model::Import> = {
        let definidora = prog.library(lib).units.first().copied();
        prog.library(lib).imports.iter().filter(|i| Some(i.unit) == definidora).collect()
    };
    let mut m = Mudanca::default();
    // O destino.
    let mut destino_b = Construtor::novo(
        uri_destino.clone(),
        Some(caminho_destino.clone()),
        texto_destino.clone().unwrap_or_default(),
        unidade_destino,
        cabecalho.map(|c| format!("{c}{eol}")),
    );
    {
        let mut t = String::new();
        if linha_antes {
            t.push_str(eol);
        }
        t.push_str(&cx.fonte[faixa_sem.0..faixa_sem.1]);
        m.adicionar(&uri_destino, Span { start: posicao, end: posicao }, t);
    }
    if analisador.tem_movendo_para_ficando()
        && let Some(u) = p.uri_da_unidade(cx.unidade)
    {
        let alvo = uri_da_biblioteca_da_unidade(cx, &u);
        destino_b.importar_biblioteca(cx, &alvo, None, None, false);
    }
    for (el, imps) in &analisador.refs_movendo {
        for &k in imps {
            let Some(imp) = imports_da_lib.get(k) else { continue };
            let alvo = prog.library(imp.library);
            if alvo.uri == "dart:core" {
                continue;
            }
            let tem_show = imp.combinators.iter().any(|c| matches!(c, ast::Combinator::Show(_)));
            let prefixo = imp.prefix.map(|x| p.nome(x).to_string());
            let nome = nome_do_elemento(cx, *el);
            destino_b.importar_biblioteca(cx, &alvo.uri.clone(), prefixo.as_deref(), nome.as_deref(), tem_show);
        }
    }
    destino_b.finalizar(cx, &mut m, eol);
    // A origem.
    let mut origem_b = Construtor::novo(uri.to_string(), prog.unit(cx.unidade).path.clone(), cx.fonte.to_string(), Some(cx.unidade), None);
    if analisador.tem_ficando_para_movendo() {
        origem_b.importar_biblioteca(cx, &import_destino, None, None, false);
    }
    m.adicionar(uri, Span { start: faixa_com.0, end: faixa_com.1 }, "");
    origem_b.finalizar(cx, &mut m, eol);
    // As outras bibliotecas que referem os movidos.
    let mut por_biblioteca: Vec<(LibraryId, Vec<String>)> = Vec::new();
    for el in &analisador.movendo {
        for (b, prefixos) in prefixos_usados(cx, *el) {
            if b == lib {
                continue;
            }
            let i = match por_biblioteca.iter().position(|(x, _)| *x == b) {
                Some(i) => i,
                None => {
                    por_biblioteca.push((b, Vec::new()));
                    por_biblioteca.len() - 1
                }
            };
            for pf in prefixos {
                if !por_biblioteca[i].1.contains(&pf) {
                    por_biblioteca[i].1.push(pf);
                }
            }
        }
    }
    for (b, prefixos) in por_biblioteca {
        let Some(&u) = prog.library(b).units.first() else { continue };
        let Some(uri_b) = p.uri_da_unidade(u) else { continue };
        let mut cb = Construtor::novo(uri_b, prog.unit(u).path.clone(), prog.unit(u).source.clone(), Some(u), None);
        for pf in &prefixos {
            cb.importar_biblioteca(cx, &import_destino, Some(pf), None, false);
        }
        cb.finalizar(cx, &mut m, eol);
    }
    // O arquivo novo: a criação, com o conteúdo das edições dele.
    if !existe {
        let edicoes = m.arquivos.iter().position(|(u, _)| *u == uri_destino).map(|i| m.arquivos.remove(i).1).unwrap_or_default();
        let mut conteudo = String::new();
        // As edições estão em ordem decrescente (no mesmo offset, a nova
        // antes): aplicadas em sequência sobre o texto vazio.
        for e in edicoes.iter() {
            conteudo.insert_str(e.span.start.min(conteudo.len()), &e.texto);
        }
        m.criar = Some((uri_destino, conteudo));
    }
    m.resultado()
}

/// A marca de uma `FunctionDeclaration` de topo é a da função.
fn funcao_de_topo(cx: &Contexto<'_>, marca: crate::arvore_analyzer::Marca, d: ast::DeclId) -> bool {
    match (marca, &cx.ast.decl(d).kind) {
        (crate::arvore_analyzer::Marca::Funcao(f), DeclKind::Function(g)) => f == *g,
        _ => false,
    }
}

/// A URI da biblioteca dona da unidade (`unitResult.uri`: a da própria
/// unidade, também numa parte).
fn uri_da_biblioteca_da_unidade(cx: &Contexto<'_>, uri_da_unidade: &str) -> String {
    let prog = cx.p.programa();
    let lib = prog.unit(cx.unidade).library;
    if prog.library(lib).units.first() == Some(&cx.unidade) {
        return prog.library(lib).uri.clone();
    }
    match url::Url::parse(uri_da_unidade).ok().and_then(|u| u.to_file_path().ok()) {
        Some(c) => uri_do_caminho(&c),
        None => uri_da_unidade.to_string(),
    }
}

/// O nome de um elemento de topo.
fn nome_do_elemento(cx: &Contexto<'_>, el: Element) -> Option<String> {
    let p = cx.p;
    let prog = p.programa();
    Some(match el {
        Element::Class(c) => p.nome(prog.class(c).name).to_string(),
        Element::Typedef(t) => p.nome(prog.typedef(t).name).to_string(),
        Element::Extension(x) => p.nome(prog.extension(x).name?).to_string(),
        Element::Function(f) => p.nome(prog.function(f).name).trim_end_matches('=').to_string(),
        Element::Variable(v) => p.nome(prog.variable(v).name).to_string(),
        Element::Prefix(..) => return None,
    })
}

/// O elemento interessante (`isInterestingReference`) de uma resolução: de
/// topo, sem prefixo; o acessor sintético vira a variável; o membro de
/// instância de extensão vira a extensão.
fn elemento_interessante(cx: &Contexto<'_>, r: &Resolved) -> Option<Element> {
    let prog = cx.p.programa();
    match r {
        Resolved::Element(Element::Prefix(..)) => None,
        Resolved::Element(Element::Function(f)) => {
            let fe = prog.function(*f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => Some(Element::Variable(v)),
                (FunctionKind::ImplicitAccessor, None) => None,
                _ => Some(Element::Function(*f)),
            }
        }
        Resolved::Element(el) => Some(*el),
        Resolved::ExtensionMember { extension, member } => (!prog.function(*member).static_).then_some(Element::Extension(*extension)),
        Resolved::Constructor(f) => prog.function(*f).class.map(Element::Class),
        _ => None,
    }
}

/// `_getImportForElement`: o import (pelo prefixo usado) que dá o elemento.
fn import_do_elemento(cx: &Contexto<'_>, unidade: UnitId, el: Element, prefixo: Option<dartforge_intern::SymbolId>) -> Option<usize> {
    let p = cx.p;
    let prog = p.programa();
    let lib = prog.unit(unidade).library;
    let definidora = prog.library(lib).units.first().copied();
    let imports: Vec<&dartforge_elements::model::Import> = prog.library(lib).imports.iter().filter(|i| Some(i.unit) == definidora).collect();
    let nome = nome_do_elemento(cx, el)?;
    let fornece = |imp: &dartforge_elements::model::Import| -> bool {
        let alvo = prog.library(imp.library);
        let Some(b) = alvo.exported.iter().find(|(s, _)| p.nome(**s) == nome).map(|(_, b)| *b) else { return false };
        if b.getter != Some(el) && b.setter != Some(el) {
            return false;
        }
        imp.combinators.iter().all(|c| match c {
            ast::Combinator::Show(ns) => ns.iter().any(|n| p.nome(n.sym) == nome),
            ast::Combinator::Hide(ns) => !ns.iter().any(|n| p.nome(n.sym) == nome),
        })
    };
    let achado = imports.iter().position(|i| i.prefix == prefixo && fornece(i));
    if achado.is_none() && prefixo.is_none() && matches!(el, Element::Extension(_)) {
        return imports.iter().position(|i| fornece(i));
    }
    achado
}

/// O `ImportAnalyzer` da biblioteca da unidade, com a faixa movida.
fn analisar_imports(cx: &Contexto<'_>, faixa: (usize, usize)) -> AnalisadorDeImports {
    let p = cx.p;
    let prog = p.programa();
    let lib = prog.unit(cx.unidade).library;
    let mut a = AnalisadorDeImports::default();
    let movido = |u: UnitId, o: usize| u == cx.unidade && faixa.0 <= o && o <= faixa.1;
    for &u in prog.library(lib).units.iter() {
        let unidade = prog.unit(u);
        let ast = &unidade.ast;
        let corpos = &p.consulta.corpos.units[u.0 as usize];
        // As declarações (pelo offset do nome).
        for &d in unidade.unit.declarations.iter() {
            let decl = ast.decl(d);
            let mut registrar = |el: Element, nome: usize| {
                if movido(u, nome) {
                    a.movendo.push(el);
                } else {
                    a.ficando.push(el);
                }
            };
            match &decl.kind {
                DeclKind::Class(c) => {
                    if let Some(id) = cx.classe_da_declaracao(u, d) {
                        registrar(Element::Class(id), c.name.span.start);
                    }
                }
                DeclKind::Mixin(m) => {
                    if let Some(id) = cx.classe_da_declaracao(u, d) {
                        registrar(Element::Class(id), m.name.span.start);
                    }
                }
                DeclKind::Enum(e) => {
                    if let Some(id) = cx.classe_da_declaracao(u, d) {
                        registrar(Element::Class(id), e.name.span.start);
                    }
                }
                DeclKind::Typedef(t) => {
                    if let Some(i) = prog.typedefs.iter().position(|x| x.decl.unit == u && x.decl.decl == d) {
                        registrar(Element::Typedef(dartforge_elements::model::TypedefId(i as u32)), t.name.span.start);
                    }
                }
                DeclKind::Extension(x) => {
                    if let Some(i) = prog.extensions.iter().position(|e| e.decl.unit == u && e.decl.decl == d) {
                        let id = dartforge_elements::model::ExtensionId(i as u32);
                        let nome = x.name.map_or(decl.span.start, |n| n.span.start);
                        registrar(Element::Extension(id), nome);
                    }
                }
                DeclKind::Function(f) => {
                    if let Some(fe) = p.funcao_do_no(u, *f)
                        && let Some(n) = ast.function(*f).name
                    {
                        registrar(Element::Function(fe), n.span.start);
                    }
                }
                DeclKind::Variables(l) => {
                    for (k, v) in l.variables.iter().enumerate() {
                        let id = prog.variables.iter().position(|x| {
                            x.node == dartforge_elements::model::VariableRef::TopLevel { unit: u, decl: d, index: k }
                        });
                        if let Some(i) = id {
                            registrar(Element::Variable(dartforge_elements::model::VariableId(i as u32)), v.name.span.start);
                        }
                    }
                }
                DeclKind::ExtensionType(_) => {}
            }
        }
        // As referências nas expressões.
        for (i, e) in ast.exprs.iter().enumerate() {
            let x = ExprId(i as u32);
            let Some(r) = corpos.get_resolved(x) else { continue };
            if matches!(r, Resolved::Member { member: MemberRef::Function(_) | MemberRef::Variable(_), .. }) {
                continue;
            }
            let Some(el) = elemento_interessante(cx, r) else { continue };
            let (offset, prefixo) = match &e.kind {
                ExprKind::Identifier(n) => (n.span.start, None),
                ExprKind::Property { target, name, .. } => {
                    let pfx = match &ast.expr(*target).kind {
                        ExprKind::Identifier(pn) if matches!(corpos.get_resolved(*target), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) => Some(pn.sym),
                        _ => None,
                    };
                    (name.span.start, pfx)
                }
                _ => (e.span.start, None),
            };
            let imp = import_do_elemento(cx, u, el, prefixo);
            if movido(u, offset) {
                AnalisadorDeImports::registrar(&mut a.refs_movendo, el, imp);
            } else {
                AnalisadorDeImports::registrar(&mut a.refs_ficando, el, imp);
            }
        }
        // As referências nos tipos escritos.
        for t in ast.types.iter() {
            let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
            let (el, prefixo) = match &name[..] {
                [n] => (prog.lookup(unidade.library, n.sym).and_then(|b| b.getter), None),
                [pf, n] => (prog.lookup_prefixed(unidade.library, pf.sym, n.sym).and_then(|b| b.getter), Some(pf.sym)),
                _ => (None, None),
            };
            let Some(el) = el else { continue };
            if matches!(el, Element::Prefix(..) | Element::Function(_) | Element::Variable(_)) {
                continue;
            }
            let imp = import_do_elemento(cx, u, el, prefixo);
            if movido(u, t.span.start) {
                AnalisadorDeImports::registrar(&mut a.refs_movendo, el, imp);
            } else {
                AnalisadorDeImports::registrar(&mut a.refs_ficando, el, imp);
            }
        }
    }
    // Sem as referências que ficam no mesmo arquivo.
    let movendo = a.movendo.clone();
    let ficando = a.ficando.clone();
    a.refs_movendo.retain(|(e, _)| !movendo.contains(e));
    a.refs_ficando.retain(|(e, _)| !ficando.contains(e));
    a
}

/// As bibliotecas (fora a de origem) que referem `el`, com os prefixos
/// usados (`searchPrefixesUsedInLibrary`; `''` sem prefixo).
fn prefixos_usados(cx: &Contexto<'_>, el: Element) -> Vec<(LibraryId, Vec<String>)> {
    let p = cx.p;
    let prog = p.programa();
    let mut v: Vec<(LibraryId, Vec<String>)> = Vec::new();
    let mut por = |b: LibraryId, pf: String| {
        let i = match v.iter().position(|(x, _)| *x == b) {
            Some(i) => i,
            None => {
                v.push((b, Vec::new()));
                v.len() - 1
            }
        };
        if !v[i].1.contains(&pf) {
            v[i].1.push(pf);
        }
    };
    let mut bibliotecas: Vec<LibraryId> = p.bibliotecas.iter().copied().collect();
    bibliotecas.sort_by_key(|b| b.0);
    for b in bibliotecas {
        for &u in prog.library(b).units.iter() {
            let unidade = prog.unit(u);
            let ast = &unidade.ast;
            let corpos = &p.consulta.corpos.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                let x = ExprId(i as u32);
                let Some(r) = corpos.get_resolved(x) else { continue };
                if elemento_interessante(cx, r) != Some(el) {
                    continue;
                }
                let pf = match &e.kind {
                    ExprKind::Property { target, .. } => match &ast.expr(*target).kind {
                        ExprKind::Identifier(pn)
                            if matches!(corpos.get_resolved(*target), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) =>
                        {
                            p.nome(pn.sym).to_string()
                        }
                        _ => String::new(),
                    },
                    _ => String::new(),
                };
                por(b, pf);
            }
            for t in ast.types.iter() {
                let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
                let (achado, pf) = match &name[..] {
                    [n] => (prog.lookup(unidade.library, n.sym).and_then(|x| x.getter), String::new()),
                    [pn, n] => (prog.lookup_prefixed(unidade.library, pn.sym, n.sym).and_then(|x| x.getter), p.nome(pn.sym).to_string()),
                    _ => (None, String::new()),
                };
                if achado == Some(el) {
                    por(b, pf);
                }
            }
        }
    }
    v
}
