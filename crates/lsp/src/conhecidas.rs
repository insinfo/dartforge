//! As bibliotecas conhecidas e o namespace de exportação delas, para o
//! `ImportLibrary` (docs/LSP-ESPECIFICACAO.md §13.7.4.1): o
//! `getTopLevelDeclarations` (`AN:src/services/top_level_declarations.dart`)
//! sobre os arquivos que o `FileStateFilter` deixa passar
//! (`AN:src/dart/analysis/file_state_filter.dart`), e o
//! `getExportedElement` sobre o `exportNamespace` (declarações da biblioteca
//! e das partes, mais os `export` com `show`/`hide`).
//!
//! Cada arquivo é resumido uma vez (nomes de topo e espécies, partes,
//! exports) e o resumo é guardado pela impressão (versão do texto aberto ou
//! data e tamanho no disco). A ordem dos candidatos é: SDK, projeto,
//! dependências (o `knownFiles` do analyzer não tem ordem reproduzível).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::DocumentStore;
use dartforge_elements::config::PackageConfig;
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use url::Url;

/// O `ElementKind` de um elemento de topo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Especie {
    Classe,
    Enum,
    TipoDeExtensao,
    AliasDeFuncao,
    AliasDeTipo,
    Mixin,
    Funcao,
    Variavel,
    Extensao,
}

/// As espécies do `forType`.
pub(crate) const DE_TIPO: &[Especie] = &[Especie::Classe, Especie::Enum, Especie::TipoDeExtensao, Especie::AliasDeFuncao, Especie::Mixin, Especie::AliasDeTipo];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Impressao {
    Aberto(i32, usize),
    Disco(Option<(SystemTime, u64)>),
}

/// Um nome público de topo: a espécie, o início da declaração e, numa
/// função, se a lista de parâmetros é vazia.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Declarado {
    pub especie: Especie,
    pub inicio: usize,
    pub sem_parametros: bool,
}

/// O resumo de um arquivo.
#[derive(Debug, Clone, Default)]
struct Resumo {
    /// Nomes públicos de topo (um setter entra pelo nome-base).
    declarados: HashMap<String, Declarado>,
    /// Os nomes de `declarados` na ordem das declarações.
    ordem: Vec<String>,
    /// As URIs das partes, como escritas.
    partes: Vec<String>,
    /// As URIs exportadas, com os combinadores (`true` = `show`).
    exports: Vec<(String, Vec<(bool, Vec<String>)>)>,
    e_parte: bool,
}

fn resumir(texto: &str) -> Resumo {
    let mut nomes = dartforge_intern::Interner::new();
    let a = dartforge_frontend::parser::parse(texto, &mut nomes);
    let mut r = Resumo::default();
    let mut por = |n: Option<ast::Name>, especie: Especie, inicio: usize, sem_parametros: bool, r: &mut Resumo| {
        if let Some(n) = n {
            let s = nomes.resolve(n.sym).to_string();
            if !s.starts_with('_') && !r.declarados.contains_key(&s) {
                r.ordem.push(s.clone());
                r.declarados.insert(s, Declarado { especie, inicio, sem_parametros });
            }
        }
    };
    for &d in &a.unit.declarations {
        let decl = a.ast.decl(d);
        let inicio = decl.span.start;
        match &decl.kind {
            DeclKind::Class(c) => por(Some(c.name), Especie::Classe, inicio, false, &mut r),
            DeclKind::Mixin(m) => por(Some(m.name), Especie::Mixin, inicio, false, &mut r),
            DeclKind::Enum(e) => por(Some(e.name), Especie::Enum, inicio, false, &mut r),
            DeclKind::ExtensionType(e) => por(Some(e.name), Especie::TipoDeExtensao, inicio, false, &mut r),
            DeclKind::Typedef(t) => {
                let e = if matches!(t.kind, ast::TypedefKind::Legacy { .. }) { Especie::AliasDeFuncao } else { Especie::AliasDeTipo };
                por(Some(t.name), e, inicio, false, &mut r)
            }
            DeclKind::Extension(x) => por(x.name, Especie::Extensao, inicio, false, &mut r),
            DeclKind::Function(f) => {
                let func = a.ast.function(*f);
                let (e, sem) = match func.kind {
                    ast::FunctionKind::Getter | ast::FunctionKind::Setter => (Especie::Variavel, false),
                    _ => (Especie::Funcao, func.parameters.as_ref().is_none_or(|p| p.is_empty())),
                };
                por(func.name, e, inicio, sem, &mut r)
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    por(Some(v.name), Especie::Variavel, inicio, false, &mut r);
                }
            }
        }
    }
    let valor = |s: &ast::StringLit| dartforge_elements::load::string_lit_value(s).map(|x| x.to_string());
    for d in &a.unit.directives {
        match &d.kind {
            DirectiveKind::Part { uri } => r.partes.extend(valor(uri)),
            DirectiveKind::PartOf { .. } => r.e_parte = true,
            DirectiveKind::Export { uri, combinators, .. } => {
                if let Some(u) = valor(uri) {
                    let cs = combinators
                        .iter()
                        .map(|c| match c {
                            ast::Combinator::Show(ns) => (true, ns.iter().map(|n| nomes.resolve(n.sym).to_string()).collect()),
                            ast::Combinator::Hide(ns) => (false, ns.iter().map(|n| nomes.resolve(n.sym).to_string()).collect()),
                        })
                        .collect();
                    r.exports.push((u, cs));
                }
            }
            _ => {}
        }
    }
    r
}

/// Uma biblioteca candidata: o arquivo e a URI dela.
#[derive(Debug, Clone)]
pub(crate) struct Candidata {
    pub caminho: PathBuf,
    pub uri: String,
    pub sdk: bool,
}

/// O índice dos resumos por arquivo.
#[derive(Debug, Default)]
pub(crate) struct IndiceDeBibliotecas {
    resumos: HashMap<PathBuf, (Impressao, Resumo)>,
}

/// O resolvedor de URIs de diretivas: `dart:` pelo SDK, `package:` pelo
/// `package_config`, relativas pela pasta do arquivo.
pub(crate) struct Resolvedor<'a> {
    pub sdk: Option<&'a SdkLayout>,
    pub pacotes: Option<PackageConfig>,
}

impl Resolvedor<'_> {
    pub(crate) fn caminho(&self, de: &Path, uri: &str) -> Option<PathBuf> {
        if let Some(nome) = uri.strip_prefix("dart:") {
            return self.sdk?.libraries.get(nome).map(|l| l.path.clone());
        }
        if uri.starts_with("package:") {
            return self.pacotes.as_ref()?.resolve_package_uri(uri).ok();
        }
        if uri.starts_with("file:") {
            return Url::parse(uri).ok()?.to_file_path().ok();
        }
        Url::from_file_path(de).ok()?.join(uri).ok()?.to_file_path().ok()
    }
}

impl IndiceDeBibliotecas {
    fn resumo(&mut self, caminho: &Path, documentos: &DocumentStore) -> Option<&Resumo> {
        let uri = Url::from_file_path(caminho).ok();
        let aberto = uri.as_ref().and_then(|u| Some((documentos.version(u.as_str())?, documentos.get(u.as_str())?)));
        let impressao = match aberto {
            Some((v, t)) => Impressao::Aberto(v, t.len()),
            None => Impressao::Disco(std::fs::metadata(caminho).ok().and_then(|m| Some((m.modified().ok()?, m.len())))),
        };
        let valido = self.resumos.get(caminho).is_some_and(|(i, _)| *i == impressao);
        if !valido {
            let texto = match aberto {
                Some((_, t)) => t.to_string(),
                None => std::fs::read_to_string(caminho).ok()?,
            };
            self.resumos.insert(caminho.to_path_buf(), (impressao, resumir(&texto)));
        }
        self.resumos.get(caminho).map(|(_, r)| r)
    }

    /// O arquivo é uma parte (`part of`).
    pub(crate) fn e_parte(&mut self, caminho: &Path, documentos: &DocumentStore) -> bool {
        self.resumo(caminho, documentos).is_some_and(|r| r.e_parte)
    }

    /// `exportNamespace.get(nome) ?? get('$nome=')`, com o acessor como a
    /// variável: a biblioteca que declara o elemento e a espécie.
    pub(crate) fn exportado(&mut self, biblioteca: &Path, nome: &str, resolvedor: &Resolvedor<'_>, documentos: &DocumentStore) -> Option<(PathBuf, Especie)> {
        let mut vistos = BTreeSet::new();
        self.exportado_em(biblioteca, nome, resolvedor, documentos, &mut vistos)
    }

    fn exportado_em(&mut self, biblioteca: &Path, nome: &str, resolvedor: &Resolvedor<'_>, documentos: &DocumentStore, vistos: &mut BTreeSet<PathBuf>) -> Option<(PathBuf, Especie)> {
        if !vistos.insert(biblioteca.to_path_buf()) {
            return None;
        }
        let r = self.resumo(biblioteca, documentos)?.clone();
        // As declarações da biblioteca e das partes ganham dos exports.
        if let Some(e) = r.declarados.get(nome) {
            return Some((biblioteca.to_path_buf(), e.especie));
        }
        for p in &r.partes {
            if let Some(cp) = resolvedor.caminho(biblioteca, p)
                && let Some(rp) = self.resumo(&cp, documentos)
                && let Some(e) = rp.declarados.get(nome)
            {
                return Some((biblioteca.to_path_buf(), e.especie));
            }
        }
        let mut achado = None;
        for (u, combinadores) in &r.exports {
            let passa = combinadores.iter().all(|(show, ns)| if *show { ns.iter().any(|n| n == nome) } else { !ns.iter().any(|n| n == nome) });
            if !passa {
                continue;
            }
            let Some(alvo) = resolvedor.caminho(biblioteca, u) else { continue };
            if let Some(x) = self.exportado_em(&alvo, nome, resolvedor, documentos, vistos) {
                achado = Some(x);
            }
        }
        achado
    }

    /// O `exportNamespace.definedNames` da biblioteca, em ordem: as
    /// declarações da biblioteca e das partes, depois as dos `export` (com
    /// `show`/`hide`) cujo nome ainda não entrou; cada nome com o arquivo
    /// que o declara.
    pub(crate) fn exportados(&mut self, biblioteca: &Path, resolvedor: &Resolvedor<'_>, documentos: &DocumentStore) -> Vec<(String, PathBuf, Declarado)> {
        let mut saida = Vec::new();
        let mut nomes = HashSet::new();
        let mut vistos = BTreeSet::new();
        self.exportados_em(biblioteca, resolvedor, documentos, &mut vistos, &|_: &str| true, &mut saida, &mut nomes);
        saida
    }

    #[allow(clippy::too_many_arguments)]
    fn exportados_em(
        &mut self,
        biblioteca: &Path,
        resolvedor: &Resolvedor<'_>,
        documentos: &DocumentStore,
        vistos: &mut BTreeSet<PathBuf>,
        filtro: &dyn Fn(&str) -> bool,
        saida: &mut Vec<(String, PathBuf, Declarado)>,
        nomes: &mut HashSet<String>,
    ) {
        if !vistos.insert(biblioteca.to_path_buf()) {
            return;
        }
        let Some(r) = self.resumo(biblioteca, documentos).cloned() else { return };
        for n in &r.ordem {
            if filtro(n) && nomes.insert(n.clone()) {
                saida.push((n.clone(), biblioteca.to_path_buf(), r.declarados[n]));
            }
        }
        for p in &r.partes {
            let Some(cp) = resolvedor.caminho(biblioteca, p) else { continue };
            let Some(rp) = self.resumo(&cp, documentos).cloned() else { continue };
            for n in &rp.ordem {
                if filtro(n) && nomes.insert(n.clone()) {
                    saida.push((n.clone(), cp.clone(), rp.declarados[n]));
                }
            }
        }
        for (u, combinadores) in &r.exports {
            let Some(alvo) = resolvedor.caminho(biblioteca, u) else { continue };
            let passa = |n: &str| filtro(n) && combinadores.iter().all(|(show, ns)| if *show { ns.iter().any(|x| x == n) } else { !ns.iter().any(|x| x == n) });
            self.exportados_em(&alvo, resolvedor, documentos, vistos, &passa, saida, nomes);
        }
    }
}

/// As bibliotecas do SDK que o `FileStateFilter` deixa passar.
const SDK_FORA: &[&str] = &["html", "indexed_db", "js", "js_util", "svg", "web_audio", "web_gl"];

/// O arquivo está fora de `lib/`, `bin/` e `web/` do pacote.
fn fora_de_lib_bin_web(arquivo: &Path, raiz: &Path) -> bool {
    !["lib", "bin", "web"].iter().any(|d| arquivo.starts_with(raiz.join(d)))
}

/// As dependências (e, com `dev`, as de desenvolvimento) do `pubspec.yaml`.
fn dependencias(raiz: &Path, dev: bool) -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    let Ok(texto) = std::fs::read_to_string(raiz.join("pubspec.yaml")) else { return v };
    let Ok(Some(r)) = dartforge_analise::naodart::yaml::ler(&texto) else { return v };
    let mut chaves = vec!["dependencies"];
    if dev {
        chaves.push("dev_dependencies");
    }
    for c in chaves {
        if let Some(m) = r.campo(c).and_then(|x| x.mapa()) {
            for (k, _) in m {
                if let Some(t) = k.texto() {
                    v.insert(t.to_string());
                }
            }
        }
    }
    v
}

/// Os arquivos `.dart` de `pasta`, recursivos, sem `src/` quando pedido.
fn dart_em(pasta: &Path, sem_src: bool) -> Vec<PathBuf> {
    let mut saida = Vec::new();
    let mut pilha = vec![pasta.to_path_buf()];
    while let Some(dir) = pilha.pop() {
        let Ok(entradas) = std::fs::read_dir(&dir) else { continue };
        let mut entradas: Vec<PathBuf> = entradas.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entradas.sort();
        for c in entradas.into_iter().rev() {
            if c.is_dir() {
                if sem_src && dir == pasta && c.file_name().is_some_and(|n| n == "src") {
                    continue;
                }
                pilha.push(c);
            } else if c.extension().is_some_and(|e| e == "dart") {
                saida.push(c);
            }
        }
    }
    saida.sort();
    saida
}

/// As bibliotecas candidatas para o arquivo `alvo` (o `knownFiles` com o
/// `FileStateFilter`), sem as partes.
pub(crate) fn candidatas(alvo: &Path, sdk: Option<&SdkLayout>, indice: &mut IndiceDeBibliotecas, documentos: &DocumentStore) -> Vec<Candidata> {
    let mut v = Vec::new();
    if let Some(sdk) = sdk {
        let mut nomes: Vec<&String> = sdk.libraries.keys().filter(|n| !n.starts_with('_') && !SDK_FORA.contains(&n.as_str())).collect();
        nomes.sort();
        for n in nomes {
            v.push(Candidata { caminho: sdk.libraries[n].path.clone(), uri: format!("dart:{n}"), sdk: true });
        }
    }
    let raiz = crate::projeto::raiz_do_projeto(alvo);
    let pubspec = raiz.join("pubspec.yaml").is_file();
    let pacote = crate::indice::nome_do_pacote(&raiz);
    let lib = raiz.join("lib");
    let fora = fora_de_lib_bin_web(alvo, &raiz);
    for arquivo in crate::projeto::arquivos_do_projeto(&raiz) {
        if indice.e_parte(&arquivo, documentos) {
            continue;
        }
        let uri = match (&pacote, arquivo.strip_prefix(&lib)) {
            (Some(p), Ok(rel)) => format!("package:{p}/{}", rel.to_string_lossy().replace('\\', "/")),
            _ => {
                // Sem URI `package:`: só para um alvo fora de `lib`/`bin`/`web`.
                if pubspec && !fora {
                    continue;
                }
                match Url::from_file_path(&arquivo) {
                    Ok(u) => u.to_string(),
                    Err(_) => continue,
                }
            }
        };
        v.push(Candidata { caminho: arquivo, uri, sdk: false });
    }
    // As dependências do `pubspec.yaml` (só as declaradas; sem `lib/src`).
    if pubspec
        && let Some(config) = PackageConfig::discover(alvo).and_then(|c| PackageConfig::load(&c).ok())
    {
        let deps = dependencias(&raiz, fora);
        let mut pacotes: Vec<_> = config.packages.values().filter(|p| deps.contains(&p.name) && Some(&p.name) != pacote.as_ref()).collect();
        pacotes.sort_by(|a, b| a.name.cmp(&b.name));
        for p in pacotes {
            let Ok(dir) = p.package_uri.to_file_path() else { continue };
            for arquivo in dart_em(&dir, true) {
                if indice.e_parte(&arquivo, documentos) {
                    continue;
                }
                let Ok(rel) = arquivo.strip_prefix(&dir) else { continue };
                let uri = format!("package:{}/{}", p.name, rel.to_string_lossy().replace('\\', "/"));
                v.push(Candidata { caminho: arquivo, uri, sdk: false });
            }
        }
    }
    v
}

/// `_isLibSrcPath`: componentes `lib`,`src` seguidos antes dos dois
/// últimos.
pub(crate) fn caminho_lib_src(caminho: &Path) -> bool {
    let partes: Vec<String> = caminho.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    (0..partes.len().saturating_sub(2)).any(|i| partes[i] == "lib" && partes[i + 1] == "src")
}
