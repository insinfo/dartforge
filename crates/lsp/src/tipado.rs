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

/// Resultado de uma análise para um documento aberto.
#[derive(Debug)]
pub(crate) struct Resultado {
    pub uri: String,
    /// Versão do texto analisado.
    pub versao: i32,
    /// Lista completa a publicar: sintaxe (parser) + semântica publicada.
    pub diagnosticos: Vec<Diagnostic>,
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
/// (a mesma regra do `dartforge analyze`).
fn raiz_do_pacote(arquivo: &Path) -> PathBuf {
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
        let raiz = raiz_do_pacote(&caminho);
        let mut e = self.estado();
        e.sujas.insert(raiz.clone());
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
        let raizes: Vec<PathBuf> = e.documentos.values().map(|d| d.raiz.clone()).collect();
        e.sujas.extend(raizes);
        self.partilhado.1.notify_all();
    }

    /// O documento fechou: o texto sai da cópia e os que dependiam dele são
    /// reanalisados com o disco.
    pub(crate) fn fechado(&self, uri: &str) {
        let mut e = self.estado();
        if let Some(d) = e.documentos.remove(uri)
            && e.documentos.values().any(|o| o.raiz == d.raiz)
        {
            e.sujas.insert(d.raiz);
            self.partilhado.1.notify_all();
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
        let (raiz, abertos, textos, config) = {
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
            (raiz, abertos, textos, config)
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
            analisar_pacote(motor_ref, &raiz, &abertos, &textos, &cancelado, &config)
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

/// Analisa os documentos abertos `abertos` do pacote `raiz` num programa só.
/// Vazio quando cancelada.
fn analisar_pacote(
    motor: &Motor,
    raiz: &Path,
    abertos: &[(String, i32, PathBuf, String)],
    textos: &HashMap<PathBuf, String>,
    cancelado: &dyn Fn() -> bool,
    config: &ConfiguracaoDaAnalise,
) -> Vec<Resultado> {
    // Os documentos nas pastas excluídas não são analisados: a publicação
    // deles fica vazia.
    let excluido = |c: &Path| config.excluidas.iter().any(|e| c.starts_with(e));
    let mut saida_excluidos: Vec<Resultado> = abertos
        .iter()
        .filter(|(_, _, c, _)| excluido(c))
        .map(|(u, v, _, _)| Resultado { uri: u.clone(), versao: *v, diagnosticos: Vec::new() })
        .collect();
    let abertos: Vec<(String, i32, PathBuf, String)> = abertos.iter().filter(|(_, _, c, _)| !excluido(c)).cloned().collect();
    let abertos = &abertos[..];
    if abertos.is_empty() {
        return saida_excluidos;
    }
    let mut arquivos: Vec<PathBuf> = Vec::new();
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
    let config = raiz.join(".dart_tool").join("package_config.json");
    let config = config.is_file().then_some(config);
    let Some(analise) = motor.analisar_com(raiz, &arquivos, config.as_deref(), textos, cancelado)
    else {
        return Vec::new();
    };
    let opcoes = dartforge_paridade::filtros::Opcoes::ler(raiz);
    let mut sintatico = AnalisadorSintatico::new();
    let mut saida = Vec::new();
    for (uri, versao, caminho, texto) in abertos {
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
        let mut semanticos: Vec<Diagnostic> =
            dartforge_paridade::publicaveis(arquivo, &opcoes, true)
                .into_iter()
                .filter(|(_, sintaxe)| !sintaxe)
                .map(|(d, _)| d)
                .collect();
        // `showTodos` (`_shouldSendError`): os TODOs saem com a configuração
        // (todos, ou os tipos pedidos), e sempre quando a severidade foi
        // promovida acima de INFO pelo `analysis_options.yaml`.
        {
            for (d, _) in dartforge_paridade::publicaveis(arquivo, &opcoes, false) {
                let Some(c) = d.code else { continue };
                if c.info().tipo != dartforge_diagnostics::TipoErro::Todo {
                    continue;
                }
                let mostrar = d.severity != dartforge_diagnostics::Severidade::Info
                    || config.todos
                    || config.tipos_de_todo.iter().any(|t| *t == c.info().nome.to_uppercase());
                if mostrar && !semanticos.iter().any(|x| x.span == d.span && x.code == d.code) {
                    semanticos.push(d);
                }
            }
        }
        semanticos.sort_by_key(|d| (d.span.start, d.span.end));
        diagnosticos.extend(semanticos);
        saida.push(Resultado {
            uri: uri.clone(),
            versao: *versao,
            diagnosticos,
        });
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
