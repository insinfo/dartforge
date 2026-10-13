//! Sessão semântica limitada: o último [`Project`] carregado é reaproveitado
//! pelas consultas seguintes enquanto nada do que ele leu mudou.
//!
//! Sem ela, cada `hover`, `definition`, `references`, `prepareRename` +
//! `rename` e `codeAction` recarrega o programa (as bibliotecas `dart:`
//! dominam o custo) e infere os corpos de novo, mesmo quando o editor pede
//! várias consultas sobre a mesma versão do texto (o VS Code pede
//! `codeAction` a cada movimento do cursor, e `rename` vem depois de
//! `prepareRename`). A política de memória continua explícita:
//!
//! * **uma** entrada, no máximo: o programa retido é o de uma consulta, não
//!   um cache por arquivo ou por versão;
//! * a entrada cai (o `drop` é imediato) a cada `didOpen`, `didChange` ou
//!   `didClose` — nenhuma árvore de uma versão velha fica presa;
//! * antes de reaproveitar, a chave é conferida: a versão e o tamanho de
//!   cada documento aberto, e a data e o tamanho de cada arquivo do disco
//!   que entrou na carga, mais a lista de arquivos `.dart` do projeto (um
//!   arquivo novo pode satisfazer um import);
//! * só retém o programa cuja fonte somada cabe no orçamento
//!   (`DARTFORGE_LSP_SESSAO_MIB`, em MiB de fonte; padrão
//!   [`DEFAULT_BUDGET_MIB`]; `0` desliga a sessão). O custo vivo medido é
//!   proporcional à fonte (`docs/LSP.md`, "Sessão semântica").

use crate::DocumentStore;
use crate::project::{Project, project_files};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Orçamento padrão, em MiB de fonte carregada (SDK incluído).
pub(crate) const DEFAULT_BUDGET_MIB: usize = 8;

/// Orçamento padrão da carga ampla das buscas não locais, em MiB de fonte
/// (o SDK do analyzer tem ~8 MiB; os pacotes do projeto somam o resto).
pub(crate) const DEFAULT_WIDE_BUDGET_MIB: usize = 48;

/// O que a entrada carregou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Só a biblioteca do arquivo (definição, hover, ações).
    Library(PathBuf),
    /// Todas as bibliotecas do projeto com essa raiz (referências, renomear).
    Project(PathBuf),
}

/// O que precisa continuar igual para a entrada valer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    scope: Scope,
    /// (URI, versão, tamanho) de cada documento aberto, em ordem.
    open: Vec<(String, i32, usize)>,
    /// Arquivos lidos do disco na carga, com (data, tamanho).
    disk: Vec<(PathBuf, Option<(SystemTime, u64)>)>,
    /// Arquivos `.dart` do projeto no momento da carga.
    files: Vec<PathBuf>,
}

/// Contadores da sessão (medição e testes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Consultas atendidas pela entrada retida.
    pub reused: u64,
    /// Consultas que carregaram o programa.
    pub loaded: u64,
    /// Entradas descartadas por mudança de texto ou do disco.
    pub invalidated: u64,
    /// Cargas não retidas por passar do orçamento.
    pub above_budget: u64,
    /// Bytes de fonte da entrada retida agora (0 sem entrada).
    pub retained_source: usize,
    /// Edições de um documento aplicadas à entrada retida sem recarga (só
    /// corpos mudaram, docs/LSP-ESPECIFICACAO.md §16.10 I4).
    pub body_swaps: u64,
}

/// A sessão: no máximo um programa retido.
pub(crate) struct Session {
    entry: Option<(Key, Project)>,
    budget: usize,
    pub(crate) stats: SessionStats,
}

impl Session {
    /// Sessão com o orçamento de `DARTFORGE_LSP_SESSAO_MIB` (ou o padrão).
    pub(crate) fn new() -> Self {
        let mib = std::env::var("DARTFORGE_LSP_SESSAO_MIB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(DEFAULT_BUDGET_MIB);
        Self::with_budget(mib)
    }

    /// A sessão da carga ampla (o SDK inteiro e os pacotes), com o
    /// orçamento de `DARTFORGE_LSP_AMPLO_MIB` (padrão
    /// [`DEFAULT_WIDE_BUDGET_MIB`]; `0` volta a carregar a cada busca).
    pub(crate) fn wide() -> Self {
        let mib = std::env::var("DARTFORGE_LSP_AMPLO_MIB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(DEFAULT_WIDE_BUDGET_MIB);
        Self::with_budget(mib)
    }

    /// Sessão com orçamento de `mib` MiB de fonte (`0`: nunca retém).
    pub(crate) fn with_budget(mib: usize) -> Self {
        Self {
            entry: None,
            budget: mib.saturating_mul(1024 * 1024),
            stats: SessionStats::default(),
        }
    }

    /// Descarta a entrada (um texto mudou). O programa cai aqui, não depois.
    pub(crate) fn invalidate(&mut self) {
        if self.entry.take().is_some() {
            self.stats.invalidated += 1;
            self.stats.retained_source = 0;
        }
    }

    /// `didChange` de `uri`: se a entrada retida tem a unidade e a edição só
    /// mudou corpos (classe Corpo, docs/LSP-ESPECIFICACAO.md §16.6), troca a
    /// unidade no lugar, recria as tabelas dela e a entrada continua valendo
    /// (§16.10 I4); senão a entrada cai.
    pub(crate) fn update(&mut self, documents: &DocumentStore, uri: &str) {
        if !self.swap(documents, uri) {
            self.invalidate();
        }
    }

    /// `didOpen` de `uri`: a entrada continua valendo quando só esse
    /// documento entrou na lista dos abertos e o texto dele é o que a carga
    /// já leu do disco para a unidade (abrir um arquivo no editor não
    /// recarrega o programa); senão cai.
    pub(crate) fn open(&mut self, documents: &DocumentStore, uri: &str) {
        if !self.keep(documents, uri, documents.get(uri).map(str::to_string)) {
            self.invalidate();
        }
    }

    /// `didClose` de `uri` (já fora de `documents`): a entrada continua
    /// valendo quando o texto do disco é o que a unidade tem; senão cai.
    pub(crate) fn close(&mut self, documents: &DocumentStore, uri: &str) {
        let disk = crate::project::uri_file(uri).and_then(|c| std::fs::read_to_string(c).ok());
        if !self.keep(documents, uri, disk) {
            self.invalidate();
            return;
        }
        // O arquivo passa a vir do disco: mudanças nele derrubam a entrada.
        if let Some((key, project)) = self.entry.as_mut()
            && let Some(u) = project.uri_unit(uri)
            && let Some(p) = project.program().unit(u).path.clone()
            && !key.disk.iter().any(|(q, _)| *q == p)
        {
            let m = metadata(&p);
            key.disk.push((p, m));
            key.disk.sort();
        }
    }

    /// A entrada retida vale com os abertos de agora se só `uri` entrou ou
    /// saiu da lista e a unidade dele tem `text`.
    fn keep(&mut self, documents: &DocumentStore, uri: &str, text: Option<String>) -> bool {
        let Some((key, project)) = self.entry.as_mut() else { return true };
        let Some(text) = text else { return false };
        let new = open(documents);
        let sem = |l: &[(String, i32, usize)]| l.iter().filter(|a| a.0 != uri).cloned().collect::<Vec<_>>();
        if sem(&new) != sem(&key.open) {
            return false;
        }
        let Some(u) = project.uri_unit(uri) else { return false };
        if project.program().unit(u).source != text {
            return false;
        }
        key.open = new;
        true
    }

    fn swap(&mut self, documents: &DocumentStore, uri: &str) -> bool {
        let Some((key, project)) = self.entry.as_mut() else { return false };
        let Some(text) = documents.get(uri) else { return false };
        let new = open(documents);
        // Só o documento editado pode ter mudado.
        if new.len() != key.open.len() || new.iter().zip(key.open.iter()).any(|(a, b)| a.0 != b.0 || (a.0 != uri && a != b)) {
            return false;
        }
        // Um documento fora da carga pode passar a ser dono de uma parte
        // (`part`): recarrega.
        let Some(u) = project.uri_unit(uri) else { return false };
        if project.program().unit(u).source != text {
            let inferred_bodies = project.libraries.contains(&project.program().unit(u).library);
            let mode = crate::query::SwapMode::Final { inferred_bodies, register_locals: true };
            if project.query.swap_unit(u, text, mode).is_none() {
                return false;
            }
            project.forget_unit(u);
        }
        let source: usize = project.program().units.iter().map(|u| u.source.len()).sum();
        if source > self.budget {
            return false;
        }
        key.open = new;
        self.stats.body_swaps += 1;
        self.stats.retained_source = source;
        true
    }

    /// O projeto para `scope`: o retido, se a chave ainda vale (uma entrada
    /// de projeto inteiro também serve a uma consulta de biblioteca do mesmo
    /// projeto), senão o que `load` devolve — retido se couber no
    /// orçamento, ou devolvido só para esta consulta.
    pub(crate) fn get(
        &mut self,
        scope: Scope,
        documents: &DocumentStore,
        load: impl FnOnce() -> Option<Project>,
    ) -> Option<Use<'_>> {
        let open = open(documents);
        let serve = self.entry.as_ref().is_some_and(|(key, _)| {
            let scope_serves = match (&key.scope, &scope) {
                (a, b) if a == b => true,
                (Scope::Project(root), Scope::Library(file)) => file.starts_with(root),
                _ => false,
            };
            scope_serves && key.open == open && disk_equal(key)
        });
        if serve {
            self.stats.reused += 1;
            return self.entry.as_mut().map(|(_, p)| Use::Retained(p));
        }
        self.invalidate();
        let project = load()?;
        self.stats.loaded += 1;
        let source: usize = project
            .program()
            .units
            .iter()
            .map(|u| u.source.len())
            .sum();
        if self.budget == 0 || source > self.budget {
            // Não cabe: vale só para esta consulta e cai com a resposta.
            self.stats.above_budget += 1;
            return Some(Use::Own(Box::new(project)));
        }
        let key = Key {
            disk: read_files(&project, documents),
            files: project_files(&project.root),
            scope,
            open,
        };
        self.stats.retained_source = source;
        self.entry = Some((key, project));
        self.entry.as_mut().map(|(_, p)| Use::Retained(p))
    }
}

/// Um projeto emprestado da sessão ou próprio da consulta (acima do
/// orçamento), usado do mesmo jeito. Mutável porque algumas consultas
/// (a ajuda de assinatura) internam tipos substituídos na tabela do
/// programa; a tabela só cresce enquanto a versão vale e cai com ela.
pub(crate) enum Use<'a> {
    Retained(&'a mut Project),
    Own(Box<Project>),
}

impl std::ops::Deref for Use<'_> {
    type Target = Project;

    fn deref(&self) -> &Project {
        match self {
            Use::Retained(p) => p,
            Use::Own(p) => p,
        }
    }
}

impl std::ops::DerefMut for Use<'_> {
    fn deref_mut(&mut self) -> &mut Project {
        match self {
            Use::Retained(p) => p,
            Use::Own(p) => p,
        }
    }
}

/// (URI, versão, tamanho) dos documentos abertos, em ordem.
fn open(documents: &DocumentStore) -> Vec<(String, i32, usize)> {
    let mut v: Vec<(String, i32, usize)> = documents
        .uris()
        .map(|u| {
            (
                u.to_string(),
                documents.version(u).unwrap_or(0),
                documents.get(u).map_or(0, str::len),
            )
        })
        .collect();
    v.sort();
    v
}

/// Data de modificação e tamanho de um arquivo.
fn metadata(path: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// Os arquivos que a carga leu do disco (os abertos vieram do editor).
fn read_files(
    project: &Project,
    documents: &DocumentStore,
) -> Vec<(PathBuf, Option<(SystemTime, u64)>)> {
    let mut v: Vec<(PathBuf, Option<(SystemTime, u64)>)> = project
        .program()
        .units
        .iter()
        .filter_map(|u| u.path.clone())
        .filter(|p| {
            url::Url::from_file_path(p)
                .ok()
                .is_none_or(|u| documents.get(u.as_str()).is_none())
        })
        .map(|p| {
            let m = metadata(&p);
            (p, m)
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// O disco ainda está como na carga.
fn disk_equal(key: &Key) -> bool {
    let root = match &key.scope {
        Scope::Project(r) => r.clone(),
        Scope::Library(a) => crate::project::project_root(a),
    };
    key.disk.iter().all(|(p, m)| metadata(p) == *m)
        && project_files(&root) == key.files
}
