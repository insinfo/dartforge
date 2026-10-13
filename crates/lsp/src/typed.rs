//! Diagnósticos tipados no fluxo contínuo (auditoria L01).
//!
//! A cada `didOpen`/`didChange`/`didClose`, o [`crate::Server`] publica na
//! hora os diagnósticos sintáticos (parser + verificadores locais de
//! `crates/analysis`) e pede a este trabalhador a análise completa do pacote
//! do documento. A análise é **a mesma** do `dartforge analyze`:
//! [`dartforge_parity::analysis::Engine`] (carga, outline, inferência de
//! `crates/types` e os verificadores de `crates/analysis`) seguida de
//! [`dartforge_parity::publicaveis`] (regra de publicação de
//! `verificados.txt`, `analysis_options.yaml` e `// ignore:`). Nenhuma regra
//! é duplicada aqui: só se escolhe quais arquivos analisar e se descarta o
//! que ficou velho.
//!
//! * **Um programa por pacote.** Todos os documentos abertos do pacote
//!   (mais a biblioteca dona de cada parte aberta) entram num único
//!   `Engine::analisar_com`, com os textos abertos valendo mais que o disco.
//!   Assim, editar um arquivo importado reanalisa quem o importa, direta ou
//!   transitivamente, sem grafo de dependências próprio.
//! * **Versão.** Cada resultado leva a versão do texto analisado; o servidor
//!   só o publica se ela ainda é a vigente. Resultado de versão velha é
//!   descartado, nunca sobrescreve o de uma nova.
//! * **Edições rápidas.** O pedido de um pacote é um conjunto (marcar de
//!   novo não enfileira outra análise): uma rajada de edições vira uma
//!   análise. A análise em curso consulta, entre as fases do motor, se o
//!   pacote foi marcado de novo; se foi, para e recomeça com o texto novo.
//! * **Memória.** O trabalhador retém só o [`Engine`] (layout do SDK e nomes
//!   de bibliotecas) e uma cópia do texto vigente de cada documento aberto.
//!   Programa, árvores e tabela de tipos vivem uma análise e caem com ela.
//!
//! [`Engine`]: dartforge_parity::analise::Motor

use crate::{Analyzer, SyntacticAnalyzer};
use dartforge_diagnostics::Diagnostic;
use dartforge_parity::analise::{Motor, chave, e_parte};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Resultado de uma análise para um arquivo: um documento aberto (com a
/// versão do texto analisado) ou um arquivo das raízes que não está aberto
/// (sem versão, com o texto do disco para as faixas).
#[derive(Debug)]
pub(crate) struct Result {
    pub uri: String,
    /// Versão do texto analisado; `None` num arquivo não aberto.
    pub version: Option<i32>,
    /// O texto analisado de um arquivo não aberto.
    pub text: Option<String>,
    /// Lista completa a publicar: sintaxe (parser) + semântica publicada.
    pub diagnostics: Vec<Diagnostic>,
    /// Os diagnósticos de um arquivo YAML do pacote (`analysis_options.yaml`,
    /// `pubspec.yaml`), já no JSON do analyzer (§3.2: os arquivos não-Dart
    /// validados na criação dos contextos e a cada evento do observador).
    pub yaml: Vec<dartforge_parity::json::DiagJson>,
}

/// Documento aberto como o trabalhador o vê.
struct Document {
    version: i32,
    text: String,
    path: PathBuf,
    root: PathBuf,
}

#[derive(Default)]
struct State {
    documents: HashMap<String, Document>,
    /// Pacotes (raiz) com análise pedida e ainda não iniciada.
    dirty: BTreeSet<PathBuf>,
    /// Pacote em análise agora.
    running: Option<PathBuf>,
    /// Gancho de teste: não inicia análise enquanto verdadeiro.
    paused: bool,
    shutdown: bool,
    /// `showTodos`: todos os TODOs, ou os destes tipos (em maiúsculas).
    all: bool,
    all_types: Vec<String>,
    /// As pastas excluídas da análise (`analysisExcludedFolders`).
    excluded: Vec<PathBuf>,
    /// As pastas do workspace (as raízes incluídas, §3.3).
    folders: Vec<PathBuf>,
    /// Por pacote, os arquivos alterados desde a última análise dele.
    changed: HashMap<PathBuf, BTreeSet<PathBuf>>,
    /// Pacotes cuja próxima análise publica todos os arquivos (a primeira,
    /// ou depois de mudar a configuração).
    publish_all: BTreeSet<PathBuf>,
}

type Shared = Arc<(Mutex<State>, Condvar)>;

/// Gancho chamado quando há resultado (o laço do `main` acorda com ele).
#[derive(Clone)]
pub(crate) struct Wake(pub Arc<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for Wake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Despertar")
    }
}

/// O lado do servidor: pedidos entram por aqui, resultados saem por `receive`.
pub(crate) struct Typed {
    shared: Shared,
    results: mpsc::Receiver<Result>,
}

impl std::fmt::Debug for Typed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tipado").finish_non_exhaustive()
    }
}

/// A raiz do pacote: o diretório mais próximo, subindo, com `pubspec.yaml`
/// (a mesma regra do `dartforge analyze`); sem `pubspec.yaml` até a pasta do
/// workspace que contém o arquivo, a pasta (o contexto da raiz incluída).
fn package_root(file: &Path, folders: &[PathBuf]) -> PathBuf {
    let mut current = file.parent();
    let folder = folders.iter().filter(|p| file.starts_with(p)).max_by_key(|p| p.components().count());
    while let Some(dir) = current {
        if dir.join("pubspec.yaml").is_file() {
            return dir.to_path_buf();
        }
        if folder.is_some_and(|p| dir == p.as_path()) {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    crate::project::project_root(file)
}

impl Typed {
    /// Inicia o trabalhador com o SDK em `sdk_lib` (o `lib/` do SDK).
    pub(crate) fn start(sdk_lib: PathBuf, wake: Option<Wake>) -> Option<Self> {
        let shared: Shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let (tx, rx) = mpsc::channel();
        let p = Arc::clone(&shared);
        // Corpos profundos recursam fundo: pilha própria, como no `analyze`.
        std::thread::Builder::new()
            .name("dartforge-lsp-tipado".into())
            .stack_size(1 << 30)
            .spawn(move || work(&p, &sdk_lib, &tx, wake))
            .ok()?;
        Some(Self {
            shared,
            results: rx,
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// O documento `uri` está na versão `version` com `text`: pede a análise
    /// do pacote dele (e cancela a que estiver em curso para o mesmo pacote).
    pub(crate) fn document(&self, uri: &str, version: i32, text: &str) {
        let Some(path) = url::Url::parse(uri)
            .ok()
            .filter(|u| u.scheme() == "file")
            .and_then(|u| u.to_file_path().ok())
        else {
            return;
        };
        if path.extension().is_none_or(|e| e != "dart") {
            return;
        }
        let mut e = self.state();
        let root = package_root(&path, &e.folders);
        e.dirty.insert(root.clone());
        e.changed.entry(root.clone()).or_default().insert(chave(&path));
        e.documents.insert(
            uri.to_string(),
            Document {
                version,
                text: text.to_string(),
                path,
                root,
            },
        );
        self.shared.1.notify_all();
    }

    /// A configuração da análise (`showTodos`, `analysisExcludedFolders`); os
    /// pacotes dos documentos abertos são analisados de novo.
    pub(crate) fn configure(&self, all: bool, all_types: Vec<String>, excluded: Vec<PathBuf>) {
        let mut e = self.state();
        e.all = all;
        e.all_types = all_types;
        e.excluded = excluded;
        let roots: Vec<PathBuf> = e.documents.values().map(|d| d.root.clone()).chain(folder_packages(&e.folders)).collect();
        e.publish_all.extend(roots.iter().cloned());
        e.dirty.extend(roots);
        self.shared.1.notify_all();
    }

    /// As pastas do workspace (§3.3): todo `.dart` delas é analisado e
    /// publicado (§3.2, I7 de §16.10), pacote a pacote; a primeira análise de
    /// cada pacote publica todos os arquivos.
    pub(crate) fn folders(&self, folders: Vec<PathBuf>) {
        let mut e = self.state();
        e.folders = folders;
        let packages = folder_packages(&e.folders);
        e.publish_all.extend(packages.iter().cloned());
        e.dirty.extend(packages);
        self.shared.1.notify_all();
    }

    /// Um arquivo mudou no disco (`workspace/didChangeWatchedFiles`, o
    /// observador do Dart, §3.3): um `.dart` não aberto passa a valer pelo
    /// disco, e os que dependem dele são reanalisados; o
    /// `analysis_options.yaml`, o `pubspec.yaml` e o `package_config.json`
    /// reconstroem os contextos (o pacote inteiro é analisado e publicado de
    /// novo, os YAML também).
    pub(crate) fn file_on_disk(&self, path: &Path) {
        let mut e = self.state();
        let root = package_root(path, &e.folders);
        let inside = e.folders.iter().any(|p| path.starts_with(p)) || e.documents.values().any(|o| o.root == root);
        if !inside {
            return;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(name, "analysis_options.yaml" | "pubspec.yaml" | "package_config.json") {
            e.publish_all.insert(root.clone());
            e.changed.entry(root.clone()).or_default().insert(chave(path));
            e.dirty.insert(root);
            self.shared.1.notify_all();
            return;
        }
        if path.extension().is_none_or(|x| x != "dart") {
            return;
        }
        // O documento aberto vale pelo editor.
        if e.documents.values().any(|d| chave(&d.path) == chave(path)) {
            return;
        }
        e.changed.entry(root.clone()).or_default().insert(chave(path));
        e.dirty.insert(root);
        self.shared.1.notify_all();
    }

    /// O documento fechou: o texto sai da cópia e os que dependiam dele são
    /// reanalisados com o disco.
    pub(crate) fn closed(&self, uri: &str) {
        let mut e = self.state();
        if let Some(d) = e.documents.remove(uri) {
            // Sem o texto do editor, vale o do disco: o arquivo mudou para a
            // análise (o `removeOverlay` do Dart), e os que dependem dele.
            let inside = e.folders.iter().any(|p| d.path.starts_with(p));
            if inside || e.documents.values().any(|o| o.root == d.root) {
                e.changed.entry(d.root.clone()).or_default().insert(chave(&d.path));
                e.dirty.insert(d.root);
                self.shared.1.notify_all();
            }
        }
    }

    /// Resultados prontos, sem bloquear.
    pub(crate) fn receive(&self) -> Vec<Result> {
        self.results.try_iter().collect()
    }

    /// O trabalhador está ocioso agora (nada pedido, nada em curso), sem
    /// esperar.
    pub(crate) fn idle(&self) -> bool {
        let e = self.state();
        (e.dirty.is_empty() || e.paused) && e.running.is_none()
    }

    /// Espera o trabalhador ficar ocioso (nada pedido, nada em curso), até
    /// `limit`. Verdadeiro se ficou.
    pub(crate) fn wait_idle(&self, limit: Duration) -> bool {
        let end = Instant::now() + limit;
        let mut e = self.state();
        loop {
            if (e.dirty.is_empty() || e.paused) && e.running.is_none() {
                return true;
            }
            let now = Instant::now();
            if now >= end {
                return false;
            }
            e = self
                .shared
                .1
                .wait_timeout(e, end - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    pub(crate) fn pause(&self, pause: bool) {
        self.state().paused = pause;
        self.shared.1.notify_all();
    }
}

impl Drop for Typed {
    fn drop(&mut self) {
        // Não espera a análise em curso: ela vê `shutdown` no próximo ponto
        // de cancelamento e a thread termina sozinha.
        self.state().shutdown = true;
        self.shared.1.notify_all();
    }
}

/// Laço do trabalhador: um pacote por vez, o mais antigo marcado primeiro.
fn work(
    p: &Shared,
    sdk_lib: &Path,
    tx: &mpsc::Sender<Result>,
    wake: Option<Wake>,
) {
    let (lock, sinal) = &**p;
    let block = || {
        lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut engine: Option<Motor> = None;
    loop {
        let (root, open, texts, config, changed) = {
            let mut e = block();
            while !e.shutdown && (e.dirty.is_empty() || e.paused) {
                e = sinal
                    .wait(e)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if e.shutdown {
                return;
            }
            let root = e.dirty.pop_first().expect("não vazio");
            let open: Vec<(String, i32, PathBuf, String)> = e
                .documents
                .iter()
                .filter(|(_, d)| d.root == root)
                .map(|(u, d)| (u.clone(), d.version, d.path.clone(), d.text.clone()))
                .collect();
            // Todos os abertos (de qualquer pacote) valem mais que o disco.
            let texts: HashMap<PathBuf, String> = e
                .documents
                .values()
                .map(|d| (chave(&d.path), d.text.clone()))
                .collect();
            e.running = Some(root.clone());
            let config = AnalysisConfiguration { all: e.all, all_types: e.all_types.clone(), excluded: e.excluded.clone() };
            // Os arquivos não abertos só entram com o pacote numa pasta do
            // workspace.
            let in_workspace = e.folders.iter().any(|p| root.starts_with(p) || p.starts_with(&root));
            let all = in_workspace && e.publish_all.remove(&root);
            let changed_ = e.changed.remove(&root).unwrap_or_default();
            (root, open, texts, config, (in_workspace, all, changed_))
        };
        if engine.is_none() {
            match Motor::novo(sdk_lib) {
                Ok(m) => engine = Some(m),
                Err(error) => {
                    eprintln!("[dartforge-lsp] diagnósticos tipados desligados: {error}");
                    let mut e = block();
                    e.running = None;
                    e.dirty.clear();
                    e.paused = true;
                    sinal.notify_all();
                    continue;
                }
            }
        }
        let engine_ref = engine.as_ref().expect("motor pronto");
        let cancelled = || {
            let e = block();
            e.shutdown || e.dirty.contains(&root)
        };
        let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analyze_package(engine_ref, &root, &open, &texts, &cancelled, &config, &changed)
        }));
        let results = match results {
            Ok(r) => r,
            Err(_) => {
                eprintln!(
                    "[dartforge-lsp] a análise tipada de {} entrou em pânico; ficam os diagnósticos sintáticos",
                    root.display()
                );
                Vec::new()
            }
        };
        if results.is_empty() && changed.0 {
            let mut e = block();
            if e.dirty.contains(&root) {
                e.changed.entry(root.clone()).or_default().extend(changed.2.iter().cloned());
                if changed.1 {
                    e.publish_all.insert(root.clone());
                }
            }
        }
        let happened = !results.is_empty();
        for r in results {
            let _ = tx.send(r);
        }
        {
            let mut e = block();
            e.running = None;
            sinal.notify_all();
        }
        if happened && let Some(d) = &wake {
            (d.0)();
        }
    }
}

/// O que da configuração do cliente muda a análise.
struct AnalysisConfiguration {
    all: bool,
    all_types: Vec<String>,
    excluded: Vec<PathBuf>,
}

/// Os pacotes das pastas do workspace: o de cada `.dart` delas.
fn folder_packages(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: BTreeSet<PathBuf> = BTreeSet::new();
    for p in folders {
        for a in dartforge_parity::corpus::arquivos_dart(p) {
            v.insert(package_root(&a, folders));
        }
    }
    v.into_iter().collect()
}

/// Os arquivos `affected` por uma mudança em `changed`: eles e, pelas
/// diretivas (`import`, `export`, `part`, `part of`), os que dependem deles,
/// transitivamente (os que o driver do Dart reanalisa e republica).
fn affected(files: &std::collections::BTreeMap<PathBuf, dartforge_parity::analise::Arquivo>, changed: &BTreeSet<PathBuf>, packages: Option<&dartforge_elements::PackageConfig>) -> BTreeSet<PathBuf> {
    let mut dependents: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for (path, a) in files {
        let mut names = dartforge_intern::Interner::new();
        let analyzed = dartforge_frontend::parser::parse(&a.texto, &mut names);
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        for d in &analyzed.unit.directives {
            use dartforge_frontend::ast::DirectiveKind as K;
            let lit = match &d.kind {
                K::Import { uri, .. } | K::Export { uri, .. } | K::Part { uri } => Some(uri),
                K::PartOf { uri: Some(uri), .. } => Some(uri),
                _ => None,
            };
            let Some(text) = lit.and_then(dartforge_elements::load::string_lit_value) else { continue };
            let target = if let Some(rest) = text.strip_prefix("package:") {
                let Some((package, rel)) = rest.split_once('/') else { continue };
                let Some(dir) = packages.and_then(|c| c.package_dirs.iter().find(|(n, _)| n == package).map(|(_, d)| d.clone())) else { continue };
                dir.join(rel)
            } else if text.contains(':') {
                continue;
            } else {
                base.join(&*text)
            };
            dependents.entry(chave(&target)).or_default().push(path.clone());
        }
    }
    let mut output: BTreeSet<PathBuf> = changed.iter().cloned().collect();
    let mut stack: Vec<PathBuf> = changed.iter().cloned().collect();
    while let Some(x) = stack.pop() {
        for d in dependents.get(&x).into_iter().flatten() {
            if output.insert(d.clone()) {
                stack.push(d.clone());
            }
        }
    }
    output
}

/// Analisa o pacote `root` num programa só: os documentos abertos dele e,
/// com o pacote numa pasta do workspace, todos os `.dart` que o `dart
/// analyze` veria (§3.2, I7). Os abertos sempre saem; os não abertos, na
/// primeira análise do pacote (ou depois de mudar a configuração) todos, e
/// depois os afetados pelos arquivos alterados. Vazio quando cancelada.
#[allow(clippy::type_complexity)]
fn analyze_package(
    engine: &Motor,
    root: &Path,
    open: &[(String, i32, PathBuf, String)],
    texts: &HashMap<PathBuf, String>,
    cancelled: &dyn Fn() -> bool,
    config: &AnalysisConfiguration,
    changed: &(bool, bool, BTreeSet<PathBuf>),
) -> Vec<Result> {
    let (in_workspace, all, changed) = (changed.0, changed.1, &changed.2);
    // Os documentos nas pastas excluídas não são analisados: a publicação
    // deles fica vazia.
    let excluded = |c: &Path| config.excluded.iter().any(|e| c.starts_with(e));
    let mut excluded_output: Vec<Result> = open
        .iter()
        .filter(|(_, _, c, _)| excluded(c))
        .map(|(u, v, _, _)| Result { uri: u.clone(), version: Some(*v), text: None, diagnostics: Vec::new(), yaml: Vec::new() })
        .collect();
    let open: Vec<(String, i32, PathBuf, String)> = open.iter().filter(|(_, _, c, _)| !excluded(c)).cloned().collect();
    let open = &open[..];
    let options = dartforge_parity::filtros::Opcoes::ler(root);
    // Os `.dart` do pacote que o `dart analyze` veria (fora das exclusões do
    // `analysis_options.yaml` e das `analysisExcludedFolders`).
    let of_package: Vec<PathBuf> = if in_workspace {
        dartforge_parity::projetos::arquivos(root, &options).into_iter().filter(|a| !excluded(a)).collect()
    } else {
        Vec::new()
    };
    if open.is_empty() && of_package.is_empty() {
        return excluded_output;
    }
    let mut files: Vec<PathBuf> = of_package.clone();
    for (_, _, path, text) in open {
        // Uma parte entra pela biblioteca dona (o motor não importa partes).
        if e_parte(text)
            && let Some(owning) = crate::semantic::owning_library(path, text, None)
        {
            files.push(owning);
        }
        files.push(path.clone());
    }
    files.sort_by_key(|a| chave(a));
    files.dedup_by_key(|a| chave(a));
    let package_config = root.join(".dart_tool").join("package_config.json");
    let package_config = package_config.is_file().then_some(package_config);
    let packages = package_config.as_deref().and_then(|c| dartforge_elements::PackageConfig::load(c).ok());
    let Some(analysis) = engine.analisar_com(root, &files, package_config.as_deref(), texts, cancelled)
    else {
        return Vec::new();
    };
    let mut syntactic = SyntacticAnalyzer::new();
    let mut output = Vec::new();
    // Os não abertos a publicar.
    let open_keys: BTreeSet<PathBuf> = open.iter().map(|(_, _, c, _)| chave(c)).collect();
    let publish: BTreeSet<PathBuf> = if all {
        of_package.iter().map(|a| chave(a)).collect()
    } else {
        affected(&analysis.arquivos, changed, packages.as_ref())
    };
    let mut targets: Vec<(String, Option<i32>, PathBuf, String)> = open.iter().map(|(u, v, c, t)| (u.clone(), Some(*v), c.clone(), t.clone())).collect();
    for a in &of_package {
        let k = chave(a);
        if open_keys.contains(&k) || !publish.contains(&k) {
            continue;
        }
        let Some(file_) = analysis.arquivos.get(&k) else { continue };
        let Ok(u) = url::Url::from_file_path(a) else { continue };
        targets.push((u.to_string(), None, a.clone(), file_.texto.clone()));
    }
    for (uri, version, path, text) in &targets {
        let Some(file) = analysis.arquivos.get(&chave(path)) else {
            continue;
        };
        if dartforge_parity::oraculo::relativo(path, root)
            .is_some_and(|rel| options.excluido(&rel))
        {
            continue;
        }
        // Sintaxe pelo mesmo parser do fluxo imediato (sem piscar); a
        // semântica, toda do motor, pela regra de publicação comum.
        let mut diagnostics = syntactic.syntax(uri, text);
        // Os publicados e, com o `showTodos` (`_shouldSendError`), os TODOs
        // (todos, ou os tipos pedidos; sempre quando a severidade foi
        // promovida acima de INFO pelo `analysis_options.yaml`), cada um na
        // posição em que o analisador o relatou.
        let published: Vec<Diagnostic> =
            dartforge_parity::publicaveis(file, &options, true)
                .into_iter()
                .filter(|(_, syntax)| !syntax)
                .map(|(d, _)| d)
                .collect();
        let mut semantic: Vec<Diagnostic> = Vec::new();
        for (d, syntax) in dartforge_parity::publicaveis(file, &options, false) {
            if syntax {
                continue;
            }
            let published_ = published.iter().any(|x| x.span == d.span && x.code == d.code && x.message == d.message);
            let todo = d.code.is_some_and(|c| {
                c.info().tipo == dartforge_diagnostics::TipoErro::Todo
                    && (d.severity != dartforge_diagnostics::Severidade::Info
                        || config.all
                        || config.all_types.iter().any(|t| *t == c.info().nome.to_uppercase()))
            });
            if (published_ || todo) && !semantic.iter().any(|x: &Diagnostic| x.span == d.span && x.code == d.code && x.message == d.message) {
                semantic.push(d);
            }
        }
        // Um publicado que a lista sem filtro não traz (o `IgnoreValidator`
        // relata à parte) vai no fim, como no analisador.
        for d in published {
            if !semantic.iter().any(|x| x.span == d.span && x.code == d.code && x.message == d.message) {
                semantic.push(d);
            }
        }
        // A ordem é a do `LibraryAnalyzer` (as fases, §1.2 da INFRA): o
        // servidor do Dart manda a lista na ordem de inserção (§3.6).
        diagnostics.extend(semantic);
        output.push(Result {
            uri: uri.clone(),
            version: *version,
            text: version.is_none().then(|| text.clone()),
            diagnostics,
            yaml: Vec::new(),
        });
    }
    // Os arquivos YAML do pacote: na primeira análise (e depois de mudar a
    // configuração) e quando um deles mudou no disco. O texto é o do disco,
    // como o observador do Dart o lê.
    if in_workspace {
        let options_yaml = root.join("analysis_options.yaml");
        let pubspec_yaml = root.join("pubspec.yaml");
        let changed_ = |p: &Path| all || changed.contains(&chave(p));
        let mut yaml: Vec<(PathBuf, Vec<dartforge_parity::json::DiagJson>)> = Vec::new();
        if options_yaml.is_file() && changed_(&options_yaml) {
            yaml.push((options_yaml.clone(), dartforge_parity::diagnosticos_das_opcoes(root)));
        }
        if pubspec_yaml.is_file() && (changed_(&pubspec_yaml) || changed_(&options_yaml)) {
            yaml.push((pubspec_yaml.clone(), dartforge_parity::diagnosticos_do_pubspec(root, &options)));
        }
        for (path, diagnostics) in yaml {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(u) = url::Url::from_file_path(&path) else { continue };
            output.push(Result { uri: u.to_string(), version: None, text: Some(text), diagnostics: Vec::new(), yaml: diagnostics });
        }
    }
    output.append(&mut excluded_output);
    output
}

/// O `lib/` do SDK do analisador, quando ele diagnostica com tipos.
pub(crate) fn analyzer_sdk<A: Analyzer>(analyzer: &A) -> Option<PathBuf> {
    if std::env::var("DARTFORGE_LSP_TIPADO").is_ok_and(|v| v == "0") {
        return None;
    }
    analyzer.sdk_for_diagnostics()
}
