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
pub(crate) enum Kind {
    Class,
    Enum,
    ExtensionType,
    FunctionAlias,
    TypeAlias,
    Mixin,
    Function,
    Variable,
    Extension,
}

/// As espécies do `forType`.
pub(crate) const OF_TYPE: &[Kind] = &[Kind::Class, Kind::Enum, Kind::ExtensionType, Kind::FunctionAlias, Kind::Mixin, Kind::TypeAlias];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Fingerprint {
    Open(i32, usize),
    Disk(Option<(SystemTime, u64)>),
}

/// Um nome público de topo: a espécie, o início da declaração e, numa
/// função, se a lista de parâmetros é vazia.
#[derive(Debug, Clone)]
pub(crate) struct Declared {
    pub kind: Kind,
    pub start: usize,
    pub without_parameters: bool,
    /// Numa função, a assinatura como o `detail` do completar a mostra
    /// (`(int a, int b) → int`), com os tipos como escritos (`dynamic` onde
    /// faltam): o resumo não tem os tipos resolvidos.
    pub signature: Option<std::sync::Arc<str>>,
}

/// O texto de um tipo como escrito, com os espaços normalizados.
fn written_type(text: &str, a: &ast::Ast, t: Option<ast::TypeId>) -> String {
    match t {
        Some(t) => {
            let s = a.ty(t).span;
            text.get(s.start..s.end).map_or_else(|| "dynamic".to_string(), |x| x.split_whitespace().collect::<Vec<_>>().join(" "))
        }
        None => "dynamic".to_string(),
    }
}

/// `(int a, [int b], {required int c}) → R` de uma função pela fonte.
fn written_signature(text: &str, a: &ast::Ast, f: &ast::Function) -> String {
    let name_of = |n: Option<ast::Name>| n.and_then(|n| text.get(n.span.start..n.span.end)).unwrap_or("").to_string();
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut named = Vec::new();
    for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
        let ty = if let Some(fp) = &p.function_parameters {
            let ret = written_type(text, a, p.ty);
            let params: Vec<String> = fp.iter().map(|q| written_type(text, a, q.ty)).collect();
            format!("{ret} Function({})", params.join(", "))
        } else {
            written_type(text, a, p.ty)
        };
        let item = format!("{ty} {}", name_of(p.name));
        match p.kind {
            ast::ParameterKind::Required => required.push(item),
            ast::ParameterKind::Optional => optional.push(item),
            ast::ParameterKind::Named => named.push(if p.required { format!("required {item}") } else { item }),
        }
    }
    let mut parts = required;
    if !optional.is_empty() {
        parts.push(format!("[{}]", optional.join(", ")));
    }
    if !named.is_empty() {
        parts.push(format!("{{{}}}", named.join(", ")));
    }
    format!("({}) → {}", parts.join(", "), written_type(text, a, f.return_type))
}

/// O resumo de um arquivo.
#[derive(Debug, Clone, Default)]
struct Summary {
    /// Nomes públicos de topo (um setter entra pelo nome-base).
    declared: HashMap<String, Declared>,
    /// Os nomes de `declared` na ordem das declarações.
    order: Vec<String>,
    /// As URIs das partes, como escritas.
    parts: Vec<String>,
    /// As URIs exportadas, com os combinadores (`true` = `show`).
    exports: Vec<(String, Vec<(bool, Vec<String>)>)>,
    is_part: bool,
}

fn summarize(text: &str) -> Summary {
    let mut names = dartforge_intern::Interner::new();
    let a = dartforge_frontend::parser::parse(text, &mut names);
    let mut r = Summary::default();
    let por = |n: Option<ast::Name>, kind: Kind, start: usize, without_parameters: bool, signature: Option<String>, r: &mut Summary| {
        if let Some(n) = n {
            let s = names.resolve(n.sym).to_string();
            if !s.starts_with('_') && !r.declared.contains_key(&s) {
                r.order.push(s.clone());
                r.declared.insert(s, Declared { kind, start, without_parameters, signature: signature.map(std::sync::Arc::from) });
            }
        }
    };
    for &d in &a.unit.declarations {
        let decl = a.ast.decl(d);
        let start = decl.span.start;
        match &decl.kind {
            DeclKind::Class(c) => por(Some(c.name), Kind::Class, start, false, None, &mut r),
            DeclKind::Mixin(m) => por(Some(m.name), Kind::Mixin, start, false, None, &mut r),
            DeclKind::Enum(e) => por(Some(e.name), Kind::Enum, start, false, None, &mut r),
            DeclKind::ExtensionType(e) => por(Some(e.name), Kind::ExtensionType, start, false, None, &mut r),
            DeclKind::Typedef(t) => {
                let e = if matches!(t.kind, ast::TypedefKind::Legacy { .. }) { Kind::FunctionAlias } else { Kind::TypeAlias };
                por(Some(t.name), e, start, false, None, &mut r)
            }
            DeclKind::Extension(x) => por(x.name, Kind::Extension, start, false, None, &mut r),
            DeclKind::Function(f) => {
                let func = a.ast.function(*f);
                let (e, sem, signature) = match func.kind {
                    ast::FunctionKind::Getter | ast::FunctionKind::Setter => (Kind::Variable, false, None),
                    _ => (Kind::Function, func.parameters.as_ref().is_none_or(|p| p.is_empty()), Some(written_signature(text, &a.ast, func))),
                };
                por(func.name, e, start, sem, signature, &mut r)
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    por(Some(v.name), Kind::Variable, start, false, None, &mut r);
                }
            }
        }
    }
    let value = |s: &ast::StringLit| dartforge_elements::load::string_lit_value(s).map(|x| x.to_string());
    for d in &a.unit.directives {
        match &d.kind {
            DirectiveKind::Part { uri } => r.parts.extend(value(uri)),
            DirectiveKind::PartOf { .. } => r.is_part = true,
            DirectiveKind::Export { uri, combinators, .. } => {
                if let Some(u) = value(uri) {
                    let cs = combinators
                        .iter()
                        .map(|c| match c {
                            ast::Combinator::Show(ns) => (true, ns.iter().map(|n| names.resolve(n.sym).to_string()).collect()),
                            ast::Combinator::Hide(ns) => (false, ns.iter().map(|n| names.resolve(n.sym).to_string()).collect()),
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
pub(crate) struct Candidate {
    pub path: PathBuf,
    pub uri: String,
    pub sdk: bool,
}

/// O índice dos resumos por arquivo.
#[derive(Debug, Default)]
pub(crate) struct LibraryIndex {
    summaries: HashMap<PathBuf, (Fingerprint, Summary)>,
}

/// O resolvedor de URIs de diretivas: `dart:` pelo SDK, `package:` pelo
/// `package_config`, relativas pela pasta do arquivo.
pub(crate) struct Resolver<'a> {
    pub sdk: Option<&'a SdkLayout>,
    pub packages: Option<PackageConfig>,
}

impl Resolver<'_> {
    pub(crate) fn path(&self, de: &Path, uri: &str) -> Option<PathBuf> {
        if let Some(name) = uri.strip_prefix("dart:") {
            return self.sdk?.libraries.get(name).map(|l| l.path.clone());
        }
        if uri.starts_with("package:") {
            return self.packages.as_ref()?.resolve_package_uri(uri).ok();
        }
        if uri.starts_with("file:") {
            return Url::parse(uri).ok()?.to_file_path().ok();
        }
        Url::from_file_path(de).ok()?.join(uri).ok()?.to_file_path().ok()
    }
}

impl LibraryIndex {
    fn summary(&mut self, path: &Path, documents: &DocumentStore) -> Option<&Summary> {
        let uri = Url::from_file_path(path).ok();
        let open = uri.as_ref().and_then(|u| Some((documents.version(u.as_str())?, documents.get(u.as_str())?)));
        let fingerprint = match open {
            Some((v, t)) => Fingerprint::Open(v, t.len()),
            None => Fingerprint::Disk(std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())))),
        };
        let valid = self.summaries.get(path).is_some_and(|(i, _)| *i == fingerprint);
        if !valid {
            let text = match open {
                Some((_, t)) => t.to_string(),
                None => std::fs::read_to_string(path).ok()?,
            };
            self.summaries.insert(path.to_path_buf(), (fingerprint, summarize(&text)));
        }
        self.summaries.get(path).map(|(_, r)| r)
    }

    /// O arquivo é uma parte (`part of`).
    pub(crate) fn is_part(&mut self, path: &Path, documents: &DocumentStore) -> bool {
        self.summary(path, documents).is_some_and(|r| r.is_part)
    }

    /// `exportNamespace.get(name) ?? get('$name=')`, com o acessor como a
    /// variável: a biblioteca que declara o elemento e a espécie.
    pub(crate) fn exported_entry(&mut self, library: &Path, name: &str, resolver: &Resolver<'_>, documents: &DocumentStore) -> Option<(PathBuf, Kind)> {
        let mut seen = BTreeSet::new();
        self.exported_entry_in(library, name, resolver, documents, &mut seen)
    }

    fn exported_entry_in(&mut self, library: &Path, name: &str, resolver: &Resolver<'_>, documents: &DocumentStore, seen: &mut BTreeSet<PathBuf>) -> Option<(PathBuf, Kind)> {
        if !seen.insert(library.to_path_buf()) {
            return None;
        }
        let r = self.summary(library, documents)?.clone();
        // As declarações da biblioteca e das partes ganham dos exports.
        if let Some(e) = r.declared.get(name) {
            return Some((library.to_path_buf(), e.kind));
        }
        for p in &r.parts {
            if let Some(cp) = resolver.path(library, p)
                && let Some(rp) = self.summary(&cp, documents)
                && let Some(e) = rp.declared.get(name)
            {
                return Some((library.to_path_buf(), e.kind));
            }
        }
        let mut found = None;
        for (u, combinators) in &r.exports {
            let passes = combinators.iter().all(|(show, ns)| if *show { ns.iter().any(|n| n == name) } else { !ns.iter().any(|n| n == name) });
            if !passes {
                continue;
            }
            let Some(target) = resolver.path(library, u) else { continue };
            if let Some(x) = self.exported_entry_in(&target, name, resolver, documents, seen) {
                found = Some(x);
            }
        }
        found
    }

    /// O `exportNamespace.definedNames` da biblioteca, em ordem: as
    /// declarações da biblioteca e das partes, depois as dos `export` (com
    /// `show`/`hide`) cujo nome ainda não entrou; cada nome com o arquivo
    /// que o declara.
    pub(crate) fn exported(&mut self, library: &Path, resolver: &Resolver<'_>, documents: &DocumentStore) -> Vec<(String, PathBuf, Declared)> {
        let mut output = Vec::new();
        let mut names = HashSet::new();
        let mut seen = BTreeSet::new();
        self.exported_in(library, resolver, documents, &mut seen, &|_: &str| true, &mut output, &mut names);
        output
    }

    #[allow(clippy::too_many_arguments)]
    fn exported_in(
        &mut self,
        library: &Path,
        resolver: &Resolver<'_>,
        documents: &DocumentStore,
        seen: &mut BTreeSet<PathBuf>,
        filter: &dyn Fn(&str) -> bool,
        output: &mut Vec<(String, PathBuf, Declared)>,
        names: &mut HashSet<String>,
    ) {
        if !seen.insert(library.to_path_buf()) {
            return;
        }
        let Some(r) = self.summary(library, documents).cloned() else { return };
        for n in &r.order {
            if filter(n) && names.insert(n.clone()) {
                output.push((n.clone(), library.to_path_buf(), r.declared[n].clone()));
            }
        }
        for p in &r.parts {
            let Some(cp) = resolver.path(library, p) else { continue };
            let Some(rp) = self.summary(&cp, documents).cloned() else { continue };
            for n in &rp.order {
                if filter(n) && names.insert(n.clone()) {
                    output.push((n.clone(), cp.clone(), rp.declared[n].clone()));
                }
            }
        }
        for (u, combinators) in &r.exports {
            let Some(target) = resolver.path(library, u) else { continue };
            let passes = |n: &str| filter(n) && combinators.iter().all(|(show, ns)| if *show { ns.iter().any(|x| x == n) } else { !ns.iter().any(|x| x == n) });
            self.exported_in(&target, resolver, documents, seen, &passes, output, names);
        }
    }
}

/// As bibliotecas do SDK que o `FileStateFilter` deixa passar.
const SDK_OUTSIDE: &[&str] = &["html", "indexed_db", "js", "js_util", "svg", "web_audio", "web_gl"];

/// O arquivo está fora de `lib/`, `bin/` e `web/` do pacote.
fn outside_lib_bin_web(file: &Path, root: &Path) -> bool {
    !["lib", "bin", "web"].iter().any(|d| file.starts_with(root.join(d)))
}

/// As dependências (e, com `dev`, as de desenvolvimento) do `pubspec.yaml`.
fn dependencies(root: &Path, dev: bool) -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    let Ok(text) = std::fs::read_to_string(root.join("pubspec.yaml")) else { return v };
    let Ok(Some(r)) = dartforge_analysis::naodart::yaml::ler(&text) else { return v };
    let mut keys = vec!["dependencies"];
    if dev {
        keys.push("dev_dependencies");
    }
    for c in keys {
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

/// Os arquivos `.dart` de `folder`, recursivos, sem `src/` quando pedido.
fn dart_in(folder: &Path, without_src: bool) -> Vec<PathBuf> {
    let mut output = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for c in entries.into_iter().rev() {
            if c.is_dir() {
                if without_src && dir == folder && c.file_name().is_some_and(|n| n == "src") {
                    continue;
                }
                stack.push(c);
            } else if c.extension().is_some_and(|e| e == "dart") {
                output.push(c);
            }
        }
    }
    output.sort();
    output
}

/// As bibliotecas candidatas para o arquivo `target` (o `knownFiles` com o
/// `FileStateFilter`), sem as partes.
pub(crate) fn candidates(target: &Path, sdk: Option<&SdkLayout>, index: &mut LibraryIndex, documents: &DocumentStore) -> Vec<Candidate> {
    let mut v = Vec::new();
    if let Some(sdk) = sdk {
        let mut names: Vec<&String> = sdk.libraries.keys().filter(|n| !n.starts_with('_') && !SDK_OUTSIDE.contains(&n.as_str())).collect();
        names.sort();
        for n in names {
            v.push(Candidate { path: sdk.libraries[n].path.clone(), uri: format!("dart:{n}"), sdk: true });
        }
    }
    let root = crate::project::project_root(target);
    let pubspec = root.join("pubspec.yaml").is_file();
    let package = crate::index::package_name(&root);
    let lib = root.join("lib");
    let outside = outside_lib_bin_web(target, &root);
    for file in crate::project::project_files(&root) {
        if index.is_part(&file, documents) {
            continue;
        }
        let uri = match (&package, file.strip_prefix(&lib)) {
            (Some(p), Ok(rel)) => format!("package:{p}/{}", rel.to_string_lossy().replace('\\', "/")),
            _ => {
                // Sem URI `package:`: só para um alvo fora de `lib`/`bin`/`web`.
                if pubspec && !outside {
                    continue;
                }
                match Url::from_file_path(&file) {
                    Ok(u) => u.to_string(),
                    Err(_) => continue,
                }
            }
        };
        v.push(Candidate { path: file, uri, sdk: false });
    }
    // As dependências do `pubspec.yaml` (só as declaradas; sem `lib/src`).
    if pubspec
        && let Some(config) = PackageConfig::discover(target).and_then(|c| PackageConfig::load(&c).ok())
    {
        let deps = dependencies(&root, outside);
        let mut packages: Vec<_> = config.packages.values().filter(|p| deps.contains(&p.name) && Some(&p.name) != package.as_ref()).collect();
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        for p in packages {
            let Ok(dir) = p.package_uri.to_file_path() else { continue };
            for file in dart_in(&dir, true) {
                if index.is_part(&file, documents) {
                    continue;
                }
                let Ok(rel) = file.strip_prefix(&dir) else { continue };
                let uri = format!("package:{}/{}", p.name, rel.to_string_lossy().replace('\\', "/"));
                v.push(Candidate { path: file, uri, sdk: false });
            }
        }
    }
    v
}

/// `_isLibSrcPath`: componentes `lib`,`src` seguidos antes dos dois
/// últimos.
pub(crate) fn path_lib_src(path: &Path) -> bool {
    let parts: Vec<String> = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    (0..parts.len().saturating_sub(2)).any(|i| parts[i] == "lib" && parts[i + 1] == "src")
}
