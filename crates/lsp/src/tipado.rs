//! Diagnósticos tipados no fluxo contínuo (auditoria L01).
//!
//! A cada `didOpen`/`didChange`/`didClose`, o [`crate::Servidor`] publica na
//! hora os diagnósticos sintáticos (parser + verificadores locais de
//! `crates/analise`) e pede a este trabalhador a análise completa do pacote
//! do documento. A análise é **a mesma** do `dartforge analyze`:
//! [`dartforge_paridade::analise::Motor`] (carga, outline, inferência de
//! `crates/types` e os verificadores de `crates/analise`) seguida de
//! [`dartforge_paridade::publicaveis`] (regra de publicação de
//! `verificados.txt`, `analysis_options.yaml` e `// ignore:`). Nenhuma regra
//! é duplicada aqui: só se escolhe quais arquivos analisar e se descarta o
//! que ficou velho.
//!
//! * **Um programa por pacote.** Todos os documentos abertos do pacote
//!   (mais a biblioteca dona de cada parte aberta) entram num único
//!   `Motor::analisar_com`, com os textos abertos valendo mais que o disco.
//!   Assim, editar um arquivo importado reanalisa quem o importa, direta ou
//!   transitivamente, sem grafo de dependências próprio.
//! * **Versão.** Cada resultado leva a versão do texto analisado; o servidor
//!   só o publica se ela ainda é a vigente. Resultado de versão velha é
//!   descartado, nunca sobrescreve o de uma nova.
//! * **Edições rápidas.** O pedido de um pacote é um conjunto (marcar de
//!   novo não enfileira outra análise): uma rajada de edições vira uma
//!   análise. A análise em curso consulta, entre as fases do motor, se o
//!   pacote foi marcado de novo; se foi, para e recomeça com o texto novo.
//! * **Memória.** O trabalhador retém só o [`Motor`] (layout do SDK e nomes
//!   de bibliotecas) e uma cópia do texto vigente de cada documento aberto.
//!   Programa, árvores e tabela de tipos vivem uma análise e caem com ela.
//!
//! [`Motor`]: dartforge_paridade::analise::Motor

use crate::{Analisador, AnalisadorSintatico};
use dartforge_diagnostics::Diagnostic;
use dartforge_paridade::analise::{Motor, chave, e_parte};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Resultado de uma análise para um arquivo: um documento aberto (com a
/// versão do texto analisado) ou um arquivo das raízes que não está aberto
/// (sem versão, com o texto do disco para as faixas).
#[derive(Debug)]
pub(crate) struct Resultado {
    pub uri: String,
    /// Versão do texto analisado; `None` num arquivo não aberto.
    pub versao: Option<i32>,
    /// O texto analisado de um arquivo não aberto.
    pub texto: Option<String>,
    /// Lista completa a publicar: sintaxe (parser) + semântica publicada.
    pub diagnosticos: Vec<Diagnostic>,
    /// Os diagnósticos de um arquivo YAML do pacote (`analysis_options.yaml`,
    /// `pubspec.yaml`), já no JSON do analyzer (§3.2: os arquivos não-Dart
    /// validados na criação dos contextos e a cada evento do observador).
    pub yaml: Vec<dartforge_paridade::json::DiagJson>,
}

/// Documento aberto como o trabalhador o vê.
struct Documento {
    versao: i32,
    texto: String,
    caminho: PathBuf,
    raiz: PathBuf,
}

#[derive(Default)]
struct Estado {
    documentos: HashMap<String, Documento>,
    /// Pacotes (raiz) com análise pedida e ainda não iniciada.
    sujas: BTreeSet<PathBuf>,
    /// Pacote em análise agora.
    rodando: Option<PathBuf>,
    /// Gancho de teste: não inicia análise enquanto verdadeiro.
    pausado: bool,
    encerrar: bool,
    /// `showTodos`: todos os TODOs, ou os destes tipos (em maiúsculas).
    todos: bool,
    tipos_de_todo: Vec<String>,
    /// As pastas excluídas da análise (`analysisExcludedFolders`).
    excluidas: Vec<PathBuf>,
    /// As pastas do workspace (as raízes incluídas, §3.3).
    pastas: Vec<PathBuf>,
    /// Por pacote, os arquivos alterados desde a última análise dele.
    alterados: HashMap<PathBuf, BTreeSet<PathBuf>>,
    /// Pacotes cuja próxima análise publica todos os arquivos (a primeira,
    /// ou depois de mudar a configuração).
    publicar_todos: BTreeSet<PathBuf>,
}

type Partilhado = Arc<(Mutex<Estado>, Condvar)>;

/// Gancho chamado quando há resultado (o laço do `main` acorda com ele).
#[derive(Clone)]
pub(crate) struct Despertar(pub Arc<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for Despertar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Despertar")
    }
}

/// O lado do servidor: pedidos entram por aqui, resultados saem por `receber`.
pub(crate) struct Tipado {
    partilhado: Partilhado,
    resultados: mpsc::Receiver<Resultado>,
}

impl std::fmt::Debug for Tipado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tipado").finish_non_exhaustive()
    }
}

/// A raiz do pacote: o diretório mais próximo, subindo, com `pubspec.yaml`
/// (a mesma regra do `dartforge analyze`); sem `pubspec.yaml` até a pasta do
/// workspace que contém o arquivo, a pasta (o contexto da raiz incluída).
fn raiz_do_pacote(arquivo: &Path, pastas: &[PathBuf]) -> PathBuf {
    let mut atual = arquivo.parent();
    let pasta = pastas.iter().filter(|p| arquivo.starts_with(p)).max_by_key(|p| p.components().count());
    while let Some(dir) = atual {
        if dir.join("pubspec.yaml").is_file() {
            return dir.to_path_buf();
        }
        if pasta.is_some_and(|p| dir == p.as_path()) {
            return dir.to_path_buf();
        }
        atual = dir.parent();
    }
    crate::projeto::raiz_do_projeto(arquivo)
}

impl Tipado {
    /// Inicia o trabalhador com o SDK em `sdk_lib` (o `lib/` do SDK).
    pub(crate) fn iniciar(sdk_lib: PathBuf, despertar: Option<Despertar>) -> Option<Self> {
        let partilhado: Partilhado = Arc::new((Mutex::new(Estado::default()), Condvar::new()));
        let (tx, rx) = mpsc::channel();
        let p = Arc::clone(&partilhado);
        // Corpos profundos recursam fundo: pilha própria, como no `analyze`.
        std::thread::Builder::new()
            .name("dartforge-lsp-tipado".into())
            .stack_size(1 << 30)
            .spawn(move || trabalhar(&p, &sdk_lib, &tx, despertar))
            .ok()?;
        Some(Self {
            partilhado,
            resultados: rx,
        })
    }

    fn estado(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.partilhado
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// O documento `uri` está na versão `versao` com `texto`: pede a análise
    /// do pacote dele (e cancela a que estiver em curso para o mesmo pacote).
    pub(crate) fn documento(&self, uri: &str, versao: i32, texto: &str) {
        let Some(caminho) = url::Url::parse(uri)
            .ok()
            .filter(|u| u.scheme() == "file")
            .and_then(|u| u.to_file_path().ok())
        else {
            return;
        };
        if caminho.extension().is_none_or(|e| e != "dart") {
            return;
        }
        let mut e = self.estado();
        let raiz = raiz_do_pacote(&caminho, &e.pastas);
        e.sujas.insert(raiz.clone());
        e.alterados.entry(raiz.clone()).or_default().insert(chave(&caminho));
        e.documentos.insert(
            uri.to_string(),
            Documento {
                versao,
                texto: texto.to_string(),
                caminho,
                raiz,
            },
        );
        self.partilhado.1.notify_all();
    }

    /// A configuração da análise (`showTodos`, `analysisExcludedFolders`); os
    /// pacotes dos documentos abertos são analisados de novo.
    pub(crate) fn configurar(&self, todos: bool, tipos_de_todo: Vec<String>, excluidas: Vec<PathBuf>) {
        let mut e = self.estado();
        e.todos = todos;
        e.tipos_de_todo = tipos_de_todo;
        e.excluidas = excluidas;
        let raizes: Vec<PathBuf> = e.documentos.values().map(|d| d.raiz.clone()).chain(pacotes_das_pastas(&e.pastas)).collect();
        e.publicar_todos.extend(raizes.iter().cloned());
        e.sujas.extend(raizes);
        self.partilhado.1.notify_all();
    }

    /// As pastas do workspace (§3.3): todo `.dart` delas é analisado e
    /// publicado (§3.2, I7 de §16.10), pacote a pacote; a primeira análise de
    /// cada pacote publica todos os arquivos.
    pub(crate) fn pastas(&self, pastas: Vec<PathBuf>) {
        let mut e = self.estado();
        e.pastas = pastas;
        let pacotes = pacotes_das_pastas(&e.pastas);
        e.publicar_todos.extend(pacotes.iter().cloned());
        e.sujas.extend(pacotes);
        self.partilhado.1.notify_all();
    }

    /// Um arquivo mudou no disco (`workspace/didChangeWatchedFiles`, o
    /// observador do Dart, §3.3): um `.dart` não aberto passa a valer pelo
    /// disco, e os que dependem dele são reanalisados; o
    /// `analysis_options.yaml`, o `pubspec.yaml` e o `package_config.json`
    /// reconstroem os contextos (o pacote inteiro é analisado e publicado de
    /// novo, os YAML também).
    pub(crate) fn arquivo_no_disco(&self, caminho: &Path) {
        let mut e = self.estado();
        let raiz = raiz_do_pacote(caminho, &e.pastas);
        let dentro = e.pastas.iter().any(|p| caminho.starts_with(p)) || e.documentos.values().any(|o| o.raiz == raiz);
        if !dentro {
            return;
        }
        let nome = caminho.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(nome, "analysis_options.yaml" | "pubspec.yaml" | "package_config.json") {
            e.publicar_todos.insert(raiz.clone());
            e.alterados.entry(raiz.clone()).or_default().insert(chave(caminho));
            e.sujas.insert(raiz);
            self.partilhado.1.notify_all();
            return;
        }
        if caminho.extension().is_none_or(|x| x != "dart") {
            return;
        }
        // O documento aberto vale pelo editor.
        if e.documentos.values().any(|d| chave(&d.caminho) == chave(caminho)) {
            return;
        }
        e.alterados.entry(raiz.clone()).or_default().insert(chave(caminho));
        e.sujas.insert(raiz);
        self.partilhado.1.notify_all();
    }

    /// O documento fechou: o texto sai da cópia e os que dependiam dele são
    /// reanalisados com o disco.
    pub(crate) fn fechado(&self, uri: &str) {
        let mut e = self.estado();
        if let Some(d) = e.documentos.remove(uri) {
            // Sem o texto do editor, vale o do disco: o arquivo mudou para a
            // análise (o `removeOverlay` do Dart), e os que dependem dele.
            let dentro = e.pastas.iter().any(|p| d.caminho.starts_with(p));
            if dentro || e.documentos.values().any(|o| o.raiz == d.raiz) {
                e.alterados.entry(d.raiz.clone()).or_default().insert(chave(&d.caminho));
                e.sujas.insert(d.raiz);
                self.partilhado.1.notify_all();
            }
        }
    }

    /// Resultados prontos, sem bloquear.
    pub(crate) fn receber(&self) -> Vec<Resultado> {
        self.resultados.try_iter().collect()
    }

    /// O trabalhador está ocioso agora (nada pedido, nada em curso), sem
    /// esperar.
    pub(crate) fn ocioso(&self) -> bool {
        let e = self.estado();
        (e.sujas.is_empty() || e.pausado) && e.rodando.is_none()
    }

    /// Espera o trabalhador ficar ocioso (nada pedido, nada em curso), até
    /// `limite`. Verdadeiro se ficou.
    pub(crate) fn esperar_ocioso(&self, limite: Duration) -> bool {
        let fim = Instant::now() + limite;
        let mut e = self.estado();
        loop {
            if (e.sujas.is_empty() || e.pausado) && e.rodando.is_none() {
                return true;
            }
            let agora = Instant::now();
            if agora >= fim {
                return false;
            }
            e = self
                .partilhado
                .1
                .wait_timeout(e, fim - agora)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    pub(crate) fn pausar(&self, pausar: bool) {
        self.estado().pausado = pausar;
        self.partilhado.1.notify_all();
    }
}

impl Drop for Tipado {
    fn drop(&mut self) {
        // Não espera a análise em curso: ela vê `encerrar` no próximo ponto
        // de cancelamento e a thread termina sozinha.
        self.estado().encerrar = true;
        self.partilhado.1.notify_all();
    }
}

/// Laço do trabalhador: um pacote por vez, o mais antigo marcado primeiro.
fn trabalhar(
    p: &Partilhado,
    sdk_lib: &Path,
    tx: &mpsc::Sender<Resultado>,
    despertar: Option<Despertar>,
) {
    let (trava, sinal) = &**p;
    let bloquear = || {
        trava
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut motor: Option<Motor> = None;
    loop {
        let (raiz, abertos, textos, config, alterados) = {
            let mut e = bloquear();
            while !e.encerrar && (e.sujas.is_empty() || e.pausado) {
                e = sinal
                    .wait(e)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if e.encerrar {
                return;
            }
            let raiz = e.sujas.pop_first().expect("não vazio");
            let abertos: Vec<(String, i32, PathBuf, String)> = e
                .documentos
                .iter()
                .filter(|(_, d)| d.raiz == raiz)
                .map(|(u, d)| (u.clone(), d.versao, d.caminho.clone(), d.texto.clone()))
                .collect();
            // Todos os abertos (de qualquer pacote) valem mais que o disco.
            let textos: HashMap<PathBuf, String> = e
                .documentos
                .values()
                .map(|d| (chave(&d.caminho), d.texto.clone()))
                .collect();
            e.rodando = Some(raiz.clone());
            let config = ConfiguracaoDaAnalise { todos: e.todos, tipos_de_todo: e.tipos_de_todo.clone(), excluidas: e.excluidas.clone() };
            // Os arquivos não abertos só entram com o pacote numa pasta do
            // workspace.
            let no_workspace = e.pastas.iter().any(|p| raiz.starts_with(p) || p.starts_with(&raiz));
            let todos = no_workspace && e.publicar_todos.remove(&raiz);
            let mudados = e.alterados.remove(&raiz).unwrap_or_default();
            (raiz, abertos, textos, config, (no_workspace, todos, mudados))
        };
        if motor.is_none() {
            match Motor::novo(sdk_lib) {
                Ok(m) => motor = Some(m),
                Err(erro) => {
                    eprintln!("[dartforge-lsp] diagnósticos tipados desligados: {erro}");
                    let mut e = bloquear();
                    e.rodando = None;
                    e.sujas.clear();
                    e.pausado = true;
                    sinal.notify_all();
                    continue;
                }
            }
        }
        let motor_ref = motor.as_ref().expect("motor pronto");
        let cancelado = || {
            let e = bloquear();
            e.encerrar || e.sujas.contains(&raiz)
        };
        let resultados = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analisar_pacote(motor_ref, &raiz, &abertos, &textos, &cancelado, &config, &alterados)
        }));
        let resultados = match resultados {
            Ok(r) => r,
            Err(_) => {
                eprintln!(
                    "[dartforge-lsp] a análise tipada de {} entrou em pânico; ficam os diagnósticos sintáticos",
                    raiz.display()
                );
                Vec::new()
            }
        };
        if resultados.is_empty() && alterados.0 {
            let mut e = bloquear();
            if e.sujas.contains(&raiz) {
                e.alterados.entry(raiz.clone()).or_default().extend(alterados.2.iter().cloned());
                if alterados.1 {
                    e.publicar_todos.insert(raiz.clone());
                }
            }
        }
        let houve = !resultados.is_empty();
        for r in resultados {
            let _ = tx.send(r);
        }
        {
            let mut e = bloquear();
            e.rodando = None;
            sinal.notify_all();
        }
        if houve && let Some(d) = &despertar {
            (d.0)();
        }
    }
}

/// O que da configuração do cliente muda a análise.
struct ConfiguracaoDaAnalise {
    todos: bool,
    tipos_de_todo: Vec<String>,
    excluidas: Vec<PathBuf>,
}

/// Os pacotes das pastas do workspace: o de cada `.dart` delas.
fn pacotes_das_pastas(pastas: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: BTreeSet<PathBuf> = BTreeSet::new();
    for p in pastas {
        for a in dartforge_paridade::corpus::arquivos_dart(p) {
            v.insert(raiz_do_pacote(&a, pastas));
        }
    }
    v.into_iter().collect()
}

/// Os arquivos `afetados` por uma mudança em `alterados`: eles e, pelas
/// diretivas (`import`, `export`, `part`, `part of`), os que dependem deles,
/// transitivamente (os que o driver do Dart reanalisa e republica).
fn afetados(arquivos: &std::collections::BTreeMap<PathBuf, dartforge_paridade::analise::Arquivo>, alterados: &BTreeSet<PathBuf>, pacotes: Option<&dartforge_elements::PackageConfig>) -> BTreeSet<PathBuf> {
    let mut dependentes: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for (caminho, a) in arquivos {
        let mut nomes = dartforge_intern::Interner::new();
        let analisado = dartforge_frontend::parser::parse(&a.texto, &mut nomes);
        let base = caminho.parent().map(Path::to_path_buf).unwrap_or_default();
        for d in &analisado.unit.directives {
            use dartforge_frontend::ast::DirectiveKind as K;
            let lit = match &d.kind {
                K::Import { uri, .. } | K::Export { uri, .. } | K::Part { uri } => Some(uri),
                K::PartOf { uri: Some(uri), .. } => Some(uri),
                _ => None,
            };
            let Some(texto) = lit.and_then(dartforge_elements::load::string_lit_value) else { continue };
            let alvo = if let Some(resto) = texto.strip_prefix("package:") {
                let Some((pacote, rel)) = resto.split_once('/') else { continue };
                let Some(dir) = pacotes.and_then(|c| c.package_dirs.iter().find(|(n, _)| n == pacote).map(|(_, d)| d.clone())) else { continue };
                dir.join(rel)
            } else if texto.contains(':') {
                continue;
            } else {
                base.join(&*texto)
            };
            dependentes.entry(chave(&alvo)).or_default().push(caminho.clone());
        }
    }
    let mut saida: BTreeSet<PathBuf> = alterados.iter().cloned().collect();
    let mut pilha: Vec<PathBuf> = alterados.iter().cloned().collect();
    while let Some(x) = pilha.pop() {
        for d in dependentes.get(&x).into_iter().flatten() {
            if saida.insert(d.clone()) {
                pilha.push(d.clone());
            }
        }
    }
    saida
}

/// Analisa o pacote `raiz` num programa só: os documentos abertos dele e,
/// com o pacote numa pasta do workspace, todos os `.dart` que o `dart
/// analyze` veria (§3.2, I7). Os abertos sempre saem; os não abertos, na
/// primeira análise do pacote (ou depois de mudar a configuração) todos, e
/// depois os afetados pelos arquivos alterados. Vazio quando cancelada.
#[allow(clippy::type_complexity)]
fn analisar_pacote(
    motor: &Motor,
    raiz: &Path,
    abertos: &[(String, i32, PathBuf, String)],
    textos: &HashMap<PathBuf, String>,
    cancelado: &dyn Fn() -> bool,
    config: &ConfiguracaoDaAnalise,
    alterados: &(bool, bool, BTreeSet<PathBuf>),
) -> Vec<Resultado> {
    let (no_workspace, todos, alterados) = (alterados.0, alterados.1, &alterados.2);
    // Os documentos nas pastas excluídas não são analisados: a publicação
    // deles fica vazia.
    let excluido = |c: &Path| config.excluidas.iter().any(|e| c.starts_with(e));
    let mut saida_excluidos: Vec<Resultado> = abertos
        .iter()
        .filter(|(_, _, c, _)| excluido(c))
        .map(|(u, v, _, _)| Resultado { uri: u.clone(), versao: Some(*v), texto: None, diagnosticos: Vec::new(), yaml: Vec::new() })
        .collect();
    let abertos: Vec<(String, i32, PathBuf, String)> = abertos.iter().filter(|(_, _, c, _)| !excluido(c)).cloned().collect();
    let abertos = &abertos[..];
    let opcoes = dartforge_paridade::filtros::Opcoes::ler(raiz);
    // Os `.dart` do pacote que o `dart analyze` veria (fora das exclusões do
    // `analysis_options.yaml` e das `analysisExcludedFolders`).
    let do_pacote: Vec<PathBuf> = if no_workspace {
        dartforge_paridade::projetos::arquivos(raiz, &opcoes).into_iter().filter(|a| !excluido(a)).collect()
    } else {
        Vec::new()
    };
    if abertos.is_empty() && do_pacote.is_empty() {
        return saida_excluidos;
    }
    let mut arquivos: Vec<PathBuf> = do_pacote.clone();
    for (_, _, caminho, texto) in abertos {
        // Uma parte entra pela biblioteca dona (o motor não importa partes).
        if e_parte(texto)
            && let Some(dona) = crate::semantica::biblioteca_dona(caminho, texto, None)
        {
            arquivos.push(dona);
        }
        arquivos.push(caminho.clone());
    }
    arquivos.sort_by_key(|a| chave(a));
    arquivos.dedup_by_key(|a| chave(a));
    let config_dos_pacotes = raiz.join(".dart_tool").join("package_config.json");
    let config_dos_pacotes = config_dos_pacotes.is_file().then_some(config_dos_pacotes);
    let pacotes = config_dos_pacotes.as_deref().and_then(|c| dartforge_elements::PackageConfig::load(c).ok());
    let Some(analise) = motor.analisar_com(raiz, &arquivos, config_dos_pacotes.as_deref(), textos, cancelado)
    else {
        return Vec::new();
    };
    let mut sintatico = AnalisadorSintatico::new();
    let mut saida = Vec::new();
    // Os não abertos a publicar.
    let abertos_chaves: BTreeSet<PathBuf> = abertos.iter().map(|(_, _, c, _)| chave(c)).collect();
    let publicar: BTreeSet<PathBuf> = if todos {
        do_pacote.iter().map(|a| chave(a)).collect()
    } else {
        afetados(&analise.arquivos, alterados, pacotes.as_ref())
    };
    let mut alvos: Vec<(String, Option<i32>, PathBuf, String)> = abertos.iter().map(|(u, v, c, t)| (u.clone(), Some(*v), c.clone(), t.clone())).collect();
    for a in &do_pacote {
        let k = chave(a);
        if abertos_chaves.contains(&k) || !publicar.contains(&k) {
            continue;
        }
        let Some(arq) = analise.arquivos.get(&k) else { continue };
        let Ok(u) = url::Url::from_file_path(a) else { continue };
        alvos.push((u.to_string(), None, a.clone(), arq.texto.clone()));
    }
    for (uri, versao, caminho, texto) in &alvos {
        let Some(arquivo) = analise.arquivos.get(&chave(caminho)) else {
            continue;
        };
        if dartforge_paridade::oraculo::relativo(caminho, raiz)
            .is_some_and(|rel| opcoes.excluido(&rel))
        {
            continue;
        }
        // Sintaxe pelo mesmo parser do fluxo imediato (sem piscar); a
        // semântica, toda do motor, pela regra de publicação comum.
        let mut diagnosticos = sintatico.sintaxe(uri, texto);
        // Os publicados e, com o `showTodos` (`_shouldSendError`), os TODOs
        // (todos, ou os tipos pedidos; sempre quando a severidade foi
        // promovida acima de INFO pelo `analysis_options.yaml`), cada um na
        // posição em que o analisador o relatou.
        let publicados: Vec<Diagnostic> =
            dartforge_paridade::publicaveis(arquivo, &opcoes, true)
                .into_iter()
                .filter(|(_, sintaxe)| !sintaxe)
                .map(|(d, _)| d)
                .collect();
        let mut semanticos: Vec<Diagnostic> = Vec::new();
        for (d, sintaxe) in dartforge_paridade::publicaveis(arquivo, &opcoes, false) {
            if sintaxe {
                continue;
            }
            let publicado = publicados.iter().any(|x| x.span == d.span && x.code == d.code && x.message == d.message);
            let todo = d.code.is_some_and(|c| {
                c.info().tipo == dartforge_diagnostics::TipoErro::Todo
                    && (d.severity != dartforge_diagnostics::Severidade::Info
                        || config.todos
                        || config.tipos_de_todo.iter().any(|t| *t == c.info().nome.to_uppercase()))
            });
            if (publicado || todo) && !semanticos.iter().any(|x: &Diagnostic| x.span == d.span && x.code == d.code && x.message == d.message) {
                semanticos.push(d);
            }
        }
        // Um publicado que a lista sem filtro não traz (o `IgnoreValidator`
        // relata à parte) vai no fim, como no analisador.
        for d in publicados {
            if !semanticos.iter().any(|x| x.span == d.span && x.code == d.code && x.message == d.message) {
                semanticos.push(d);
            }
        }
        // A ordem é a do `LibraryAnalyzer` (as fases, §1.2 da INFRA): o
        // servidor do Dart manda a lista na ordem de inserção (§3.6).
        diagnosticos.extend(semanticos);
        saida.push(Resultado {
            uri: uri.clone(),
            versao: *versao,
            texto: versao.is_none().then(|| texto.clone()),
            diagnosticos,
            yaml: Vec::new(),
        });
    }
    // Os arquivos YAML do pacote: na primeira análise (e depois de mudar a
    // configuração) e quando um deles mudou no disco. O texto é o do disco,
    // como o observador do Dart o lê.
    if no_workspace {
        let opcoes_yaml = raiz.join("analysis_options.yaml");
        let pubspec_yaml = raiz.join("pubspec.yaml");
        let mudou = |p: &Path| todos || alterados.contains(&chave(p));
        let mut yaml: Vec<(PathBuf, Vec<dartforge_paridade::json::DiagJson>)> = Vec::new();
        if opcoes_yaml.is_file() && mudou(&opcoes_yaml) {
            yaml.push((opcoes_yaml.clone(), dartforge_paridade::diagnosticos_das_opcoes(raiz)));
        }
        if pubspec_yaml.is_file() && (mudou(&pubspec_yaml) || mudou(&opcoes_yaml)) {
            yaml.push((pubspec_yaml.clone(), dartforge_paridade::diagnosticos_do_pubspec(raiz, &opcoes)));
        }
        for (caminho, diagnosticos) in yaml {
            let Ok(texto) = std::fs::read_to_string(&caminho) else { continue };
            let Ok(u) = url::Url::from_file_path(&caminho) else { continue };
            saida.push(Resultado { uri: u.to_string(), versao: None, texto: Some(texto), diagnosticos: Vec::new(), yaml: diagnosticos });
        }
    }
    saida.append(&mut saida_excluidos);
    saida
}

/// O `lib/` do SDK do analisador, quando ele diagnostica com tipos.
pub(crate) fn sdk_do_analisador<A: Analisador>(analisador: &A) -> Option<PathBuf> {
    if std::env::var("DARTFORGE_LSP_TIPADO").is_ok_and(|v| v == "0") {
        return None;
    }
    analisador.sdk_para_diagnosticos()
}
