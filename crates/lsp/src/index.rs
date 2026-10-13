//! Índices de nomes públicos de topo, para o completar com importação
//! automática e para a ação "importar biblioteca".
//!
//! * [`SdkIndex`]: as bibliotecas públicas do SDK (sem `_`) com as suas
//!   partes, montado uma vez por SDK; o tamanho é o do SDK.
//! * [`ProjectIndex`]: os arquivos `.dart` de **um** projeto (o último
//!   consultado), por arquivo, com a impressão de cada um (versão do texto
//!   aberto ou data e tamanho no disco). Só o arquivo que mudou é analisado
//!   de novo; arquivo que sumiu sai do índice; trocar de projeto descarta o
//!   anterior. Guarda só nomes, espécies e offsets, nunca árvores.

use crate::DocumentStore;
use crate::completion::kind;
use crate::project::{project_files, is_part};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use url::Url;

/// Um nome público de topo declarado num arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declared {
    pub name: String,
    /// `CompletionItemKind`.
    pub kind: u32,
    /// Nomeia um tipo (classe, mixin, enum, extension type, typedef).
    pub ty: bool,
    /// Função sem parâmetros (o rótulo é `f()`, não `f(…)`).
    pub without_parameters: bool,
    /// Arquivo e início da declaração (com metadados), para a documentação.
    pub file: PathBuf,
    pub start: usize,
}

/// Os nomes públicos de topo de uma unidade analisada.
pub(crate) fn declared(
    ast: &ast::Ast,
    unit: &ast::CompilationUnit,
    names: &dartforge_intern::Interner,
    file: &Path,
) -> Vec<Declared> {
    let mut output = Vec::new();
    for &d in &unit.declarations {
        let decl = ast.decl(d);
        let mut push =
            |n: Option<ast::Name>, kind: u32, ty: bool, without_parameters: bool| {
                if let Some(n) = n {
                    let s = names.resolve(n.sym);
                    if !s.starts_with('_') {
                        output.push(Declared {
                            name: s.to_string(),
                            kind,
                            ty,
                            without_parameters,
                            file: file.to_path_buf(),
                            start: decl.span.start,
                        });
                    }
                }
            };
        match &decl.kind {
            DeclKind::Class(c) => push(Some(c.name), kind::CLASS, true, false),
            DeclKind::Mixin(m) => push(Some(m.name), kind::CLASS, true, false),
            DeclKind::Enum(e) => push(Some(e.name), kind::ENUM, true, false),
            DeclKind::ExtensionType(e) => push(Some(e.name), kind::CLASS, true, false),
            DeclKind::Typedef(t) => push(Some(t.name), kind::CLASS, true, false),
            DeclKind::Extension(x) => push(x.name, kind::CLASS, false, false),
            DeclKind::Function(f) => {
                let func = ast.function(*f);
                let (space, sem) = match func.kind {
                    ast::FunctionKind::Getter | ast::FunctionKind::Setter => {
                        (kind::VARIABLE, false)
                    }
                    _ => (
                        kind::FUNCTION,
                        func.parameters.as_ref().is_none_or(|p| p.is_empty()),
                    ),
                };
                push(func.name, space, false, sem)
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    push(Some(v.name), kind::VARIABLE, false, false);
                }
            }
        }
    }
    output
}

/// Nomes públicos de topo do SDK: nome → (URI `dart:` → declaração).
#[derive(Debug, Default)]
pub(crate) struct SdkIndex {
    pub by_name: HashMap<String, BTreeMap<String, Declared>>,
}

/// Monta o índice do SDK: cada biblioteca pública (sem `_`) com as suas
/// partes.
pub(crate) fn index_sdk(sdk: &SdkLayout) -> SdkIndex {
    let mut index = SdkIndex::default();
    let mut libraries: Vec<_> = sdk
        .libraries
        .values()
        .filter(|l| !l.name.starts_with('_') && l.supported)
        .collect();
    libraries.sort_by(|a, b| a.name.cmp(&b.name));
    for lib in libraries {
        let uri = format!("dart:{}", lib.name);
        let mut pending = vec![lib.path.clone()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(path) = pending.pop() {
            if !seen.insert(path.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let mut names = dartforge_intern::Interner::new();
            let analyzed = dartforge_frontend::parser::parse(&text, &mut names);
            for d in declared(&analyzed.ast, &analyzed.unit, &names, &path) {
                index
                    .by_name
                    .entry(d.name.clone())
                    .or_default()
                    .entry(uri.clone())
                    .or_insert(d);
            }
            for d in &analyzed.unit.directives {
                if let DirectiveKind::Part { uri } = &d.kind
                    && let Some(relative) = dartforge_elements::load::string_lit_value(uri)
                    && let Some(dir) = path.parent()
                {
                    pending.push(dir.join(relative));
                }
            }
        }
    }
    index
}

/// Impressão de um arquivo: versão e tamanho do texto aberto, ou data e
/// tamanho no disco.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fingerprint {
    Open(i32, usize),
    Disk(Option<(SystemTime, u64)>),
}

#[derive(Debug)]
struct FileEntry {
    fingerprint: Fingerprint,
    /// O arquivo é uma parte (os nomes dele são da biblioteca dona e não se
    /// importam por ele).
    part: bool,
    names: Vec<Declared>,
}

/// Índice incremental dos nomes públicos de topo de um projeto.
#[derive(Debug, Default)]
pub(crate) struct ProjectIndex {
    root: Option<PathBuf>,
    files: HashMap<PathBuf, FileEntry>,
    /// Arquivos analisados na última atualização (medição e testes).
    pub(crate) reanalyzed: usize,
}

impl ProjectIndex {
    /// Atualiza o índice do projeto `root` com os textos vigentes e devolve
    /// as bibliotecas (arquivos que não são partes) com os nomes delas.
    pub(crate) fn update(
        &mut self,
        root: &Path,
        documents: &DocumentStore,
    ) -> Vec<(&Path, &[Declared])> {
        if self.root.as_deref() != Some(root) {
            self.root = Some(root.to_path_buf());
            self.files.clear();
        }
        self.reanalyzed = 0;
        let files = project_files(root);
        self.files
            .retain(|p, _| files.binary_search(p).is_ok());
        for file in &files {
            let uri = Url::from_file_path(file).ok();
            let open = uri
                .as_ref()
                .and_then(|u| Some((documents.version(u.as_str())?, documents.get(u.as_str())?)));
            let fingerprint = match open {
                Some((v, t)) => Fingerprint::Open(v, t.len()),
                None => Fingerprint::Disk(
                    std::fs::metadata(file)
                        .ok()
                        .and_then(|m| Some((m.modified().ok()?, m.len()))),
                ),
            };
            if self
                .files
                .get(file)
                .is_some_and(|e| e.fingerprint == fingerprint)
            {
                continue;
            }
            let text = match open {
                Some((_, t)) => Some(t.to_string()),
                None => std::fs::read_to_string(file).ok(),
            };
            let Some(text) = text else {
                self.files.remove(file);
                continue;
            };
            self.reanalyzed += 1;
            let part = is_part(&text);
            let mut names = dartforge_intern::Interner::new();
            let analyzed = dartforge_frontend::parser::parse(&text, &mut names);
            let declared = declared(&analyzed.ast, &analyzed.unit, &names, file);
            self.files.insert(
                file.clone(),
                FileEntry {
                    fingerprint,
                    part,
                    names: declared,
                },
            );
        }
        let mut output: Vec<(&Path, &[Declared])> = self
            .files
            .iter()
            .filter(|(_, e)| !e.part)
            .map(|(p, e)| (p.as_path(), e.names.as_slice()))
            .collect();
        output.sort_by(|a, b| a.0.cmp(b.0));
        output
    }
}

/// Caminho de `target` relativo ao diretório `base`, com `/`.
pub(crate) fn relative(target: &Path, base: &Path) -> Option<String> {
    use std::path::Component;
    let d: Vec<Component> = target.components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = d.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return None;
    }
    let mut parts: Vec<String> = b[common..].iter().map(|_| "..".to_string()).collect();
    parts.extend(
        d[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(parts.join("/"))
}

/// Nome do pacote no `pubspec.yaml` da raiz.
pub(crate) fn package_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("pubspec.yaml")).ok()?;
    text.lines().find_map(|l| {
        l.strip_prefix("name:")
            .map(|n| n.trim().trim_matches(['\'', '"']).to_string())
    })
}

/// A URI com que `file` importa `target` do mesmo projeto: `package:`
/// quando `file` está fora de `lib/` e `target` dentro; senão relativa.
pub(crate) fn import_uri(
    file: &Path,
    target: &Path,
    root: &Path,
    package: Option<&str>,
) -> Option<String> {
    let lib = root.join("lib");
    match package {
        Some(p) if target.starts_with(&lib) && !file.starts_with(&lib) => {
            relative(target, &lib).map(|r| format!("package:{p}/{r}"))
        }
        _ => file.parent().and_then(|dir| relative(target, dir)),
    }
}
