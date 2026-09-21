//! Cache da última saída por conteúdo exato e cache limitado de planos de macros.
//!
//! Cada solicitação recarrega fontes e resolução de pacotes. Um acerto economiza
//! parsing/análise/emissão, mas não leituras, descoberta do grafo ou tokenização
//! feita pelo carregador. Falhas descartam a entrada anterior.
//! Planos de macros podem sobreviver a edições sem reutilizar AST/análise/emissão.
use crate::{
    CompileOptions, CompileReport, LinkStats, MacroCacheStats, MacroSession, Optimization,
    compile_loaded_graph_instrumented,
};
use dartforge_packages::{Combinator, GraphError, Import, SourceGraph};
use std::{path::Path, sync::Arc};

/// Estatísticas de uma solicitação concluída com sucesso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStats {
    /// Planos reutilizados nesta solicitação; zero quando a saída JS inteira foi reutilizada.
    pub macro_plan_hits: usize,
    /// Planos materializados pela primeira vez ou sem retenção nesta solicitação.
    pub macro_plan_misses: usize,
    /// O grafo e a opção eram idênticos à última compilação armazenada.
    pub cache_hit: bool,
    /// Fontes carregadas nesta chamada; não inclui leituras de configuração/metadados.
    pub source_units_loaded: usize,
    /// Unidades enviadas ao pipeline; zero em acerto e todas as unidades em falha de cache.
    pub compiled_units: usize,
}

/// JavaScript compartilhável e estatísticas da solicitação.
#[derive(Debug, Clone)]
pub struct Compilation {
    /// Módulo ES completo. Arc evita copiar a saída em acertos do cache.
    pub javascript: Arc<str>,
    /// Contagens honestas desta solicitação, sem inferir granularidade incremental.
    pub stats: SessionStats,
    /// Tempo por fase e trabalho realizado; em acerto só `load_ns` é diferente de zero.
    pub report: CompileReport,
}

/// Sessão com no máximo uma entrada e orçamento de payload configurável.
///
/// O orçamento padrão é 16 MiB de texto/caminhos/URIs/nomes retidos. Exclui
/// capacidade ociosa, estruturas, Arc e overhead do alocador; não limita a memória
/// transitória da compilação nem Arcs de saída mantidos pelo chamador.
pub struct CompilerSession {
    macros: MacroSession,
    cached: Option<Cached>,
    max_payload_bytes: usize,
    disk: Option<crate::DiskCache>,
}

/// Snapshot integral e opção que produziram a saída armazenada.
struct Cached {
    graph: SourceGraph,
    options: CompileOptions,
    javascript: Arc<str>,
}

impl Default for CompilerSession {
    /// Usa uma entrada com orçamento padrão de 16 MiB de payload.
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerSession {
    /// Cria uma sessão vazia com orçamento padrão de 16 MiB.
    ///
    /// ```
    /// let mut session = dartforge_compiler::CompilerSession::new();
    /// session.clear();
    /// ```
    pub fn new() -> Self {
        Self::with_cache_limit_bytes(16 * 1024 * 1024)
    }

    /// Define orçamento de payload; zero desativa retenção sem desativar compilação.
    ///
    /// ```
    /// let _session = dartforge_compiler::CompilerSession::with_cache_limit_bytes(0);
    /// ```
    pub fn with_cache_limit_bytes(max_payload_bytes: usize) -> Self {
        Self {
            macros: MacroSession::with_limits(256, (max_payload_bytes / 4).min(1024 * 1024)),
            cached: None,
            max_payload_bytes,
            disk: None,
        }
    }

    /// Liga o cache em disco, que sobrevive ao encerramento do processo.
    ///
    /// Cobre o cenário "nova execução": o processo começa sem nada em memória e
    /// ainda assim dispensa o front-end quando nenhuma fonte mudou. A validação
    /// continua sendo por conteúdo exato — o registro guarda as fontes e as
    /// relê para comparar —, então nenhuma garantia é afrouxada.
    ///
    /// ```no_run
    /// use dartforge_compiler::{CompilerSession, DiskCache};
    /// let cache = DiskCache::new(std::path::Path::new(".dart_tool/dartforge"));
    /// let _sessao = CompilerSession::new().with_disk_cache(cache);
    /// ```
    pub fn with_disk_cache(mut self, cache: crate::DiskCache) -> Self {
        self.disk = Some(cache);
        self
    }

    /// Descarta o snapshot e a referência da sessão à saída anterior.
    ///
    /// Saídas Arc já entregues continuam válidas enquanto o chamador as mantiver.
    pub fn clear(&mut self) {
        self.cached = None;
        self.macros.clear();
    }

    /// Consulta retenção e contadores do cache de planos, independentemente do cache JS.
    pub fn macro_cache_stats(&self) -> MacroCacheStats {
        self.macros.stats()
    }

    /// Recarrega o grafo, compara conteúdo exato e reutiliza somente a mesma saída.
    ///
    /// A chave inclui caminhos canônicos, fontes, imports/exports/combinadores,
    /// entrada e opção de otimização. Não depende de mtime, tamanho ou hash.
    /// Reconfigurar pacotes é observado pelo carregador antes da comparação.
    ///
    /// # Erros
    ///
    /// Qualquer erro de carga ou compilação é retornado e limpa o cache. Uma saída
    /// antiga nunca é entregue no lugar de um erro da solicitação atual.
    ///
    /// ```no_run
    /// use dartforge_compiler::{CompilerSession, Optimization};
    /// let mut session = CompilerSession::new();
    /// let result = session.compile_path(std::path::Path::new("main.dart"), Optimization::None)?;
    /// println!("{} fontes", result.stats.source_units_loaded);
    /// # Ok::<(), dartforge_packages::GraphError>(())
    /// ```
    pub fn compile_path(
        &mut self,
        path: &Path,
        optimization: Optimization,
    ) -> Result<Compilation, GraphError> {
        self.compile_path_with_options(path, optimization.into())
    }
    /// Compila com chave de cache que inclui todos os passes opcionais.
    /// # Erros
    /// Erros de carga ou compilação invalidam a entrada anterior.
    pub fn compile_path_with_options(
        &mut self,
        path: &Path,
        options: CompileOptions,
    ) -> Result<Compilation, GraphError> {
        self.compile_path_with_environment(
            path,
            options,
            &crate::CompilationEnvironment::javascript(),
        )
    }
    /// Inclui o perfil de plataforma na chave de cache do grafo selecionado.
    ///
    /// # Erros
    /// Falhas de ambiente, carregamento ou compilação descartam a entrada anterior.
    pub fn compile_path_with_environment(
        &mut self,
        path: &Path,
        options: CompileOptions,
        environment: &crate::CompilationEnvironment,
    ) -> Result<Compilation, GraphError> {
        let total = std::time::Instant::now();
        let load = std::time::Instant::now();
        // Retira antes de carregar: inclusive erros de filesystem invalidam a entrada.
        let previous = self.cached.take();
        if let Err(error) =
            crate::validate_environment(path, environment, crate::CompilationTarget::JavaScript)
        {
            self.macros.clear();
            return Err(error);
        }
        // Sem entrada em memória, o disco pode dispensar até a descoberta do
        // grafo: o registro sabe exatamente quais arquivos ler e com o que
        // compará-los. É por isso que esta verificação vem antes do carregador.
        if previous.is_none()
            && let Some(disk) = &self.disk
            && let Some(hit) = disk.load(path, options, &format!("{:?}", environment.target()))
        {
            return Ok(Compilation {
                javascript: hit.javascript.into(),
                stats: SessionStats {
                    macro_plan_hits: 0,
                    macro_plan_misses: 0,
                    cache_hit: true,
                    source_units_loaded: hit.units,
                    compiled_units: 0,
                },
                report: CompileReport {
                    load_ns: load.elapsed().as_nanos(),
                    link: LinkStats {
                        units: hit.units,
                        source_bytes: hit.source_bytes,
                        ..LinkStats::default()
                    },
                    total_ns: total.elapsed().as_nanos(),
                    cache_hit: true,
                },
            });
        }
        // Caminho interativo: com um grafo anterior, relê os arquivos conhecidos
        // em paralelo e reaproveita a estrutura quando só os corpos mudaram.
        // A comparação continua sendo por conteúdo exato, nunca por mtime.
        let revalidated = previous
            .as_ref()
            .and_then(|cached| dartforge_packages::revalidate(&cached.graph, path, environment));
        let graph = match revalidated {
            Some(graph) => graph,
            None => match dartforge_packages::load_with_environment(path, environment) {
                Ok(graph) => graph,
                Err(error) => {
                    self.macros.clear();
                    return Err(error);
                }
            },
        };
        let load_ns = load.elapsed().as_nanos();
        let count = graph.units.len();
        if let Some(cached) = previous.as_ref()
            && cached.options == options
            && cached.graph == graph
        {
            let javascript = Arc::clone(&cached.javascript);
            self.cached = previous;
            return Ok(Compilation {
                javascript,
                stats: SessionStats {
                    macro_plan_hits: 0,
                    macro_plan_misses: 0,
                    cache_hit: true,
                    source_units_loaded: count,
                    compiled_units: 0,
                },
                report: CompileReport {
                    load_ns,
                    link: LinkStats::default(),
                    total_ns: total.elapsed().as_nanos(),
                    cache_hit: true,
                },
            });
        }
        drop(previous);
        let before = self.macros.stats();
        let mut link = LinkStats::default();
        let javascript: Arc<str> =
            match compile_loaded_graph_instrumented(&graph, options, &mut self.macros, &mut link) {
                Ok(javascript) => javascript.into(),
                Err(error) => {
                    self.macros.clear();
                    return Err(error);
                }
            };
        if let Some(disk) = &self.disk {
            disk.store(
                path,
                options,
                &format!("{:?}", environment.target()),
                &graph,
                &javascript,
            );
        }
        let after = self.macros.stats();
        if payload_bytes(&graph)
            .saturating_add(javascript.len())
            .saturating_add(after.payload_bytes)
            <= self.max_payload_bytes
        {
            self.cached = Some(Cached {
                graph,
                options,
                javascript: Arc::clone(&javascript),
            });
        }
        Ok(Compilation {
            javascript,
            stats: SessionStats {
                macro_plan_hits: after.hits.saturating_sub(before.hits),
                macro_plan_misses: after.misses.saturating_sub(before.misses),
                cache_hit: false,
                source_units_loaded: count,
                compiled_units: count,
            },
            report: CompileReport {
                load_ns,
                link,
                total_ns: total.elapsed().as_nanos(),
                cache_hit: false,
            },
        })
    }
}

/// Soma bytes textuais de uma aresta e seus filtros sem medir memória do processo.
fn import_payload(import: &Import) -> usize {
    import
        .combinators
        .iter()
        .fold(import.uri.len(), |total, combinator| {
            let (Combinator::Show(names) | Combinator::Hide(names)) = combinator;
            names
                .iter()
                .fold(total, |total, name| total.saturating_add(name.len()))
        })
}

/// Calcula o payload retido; somas saturadas impedem orçamento contornado por overflow.
fn payload_bytes(graph: &SourceGraph) -> usize {
    graph.units.iter().fold(0usize, |total, unit| {
        let total = total
            .saturating_add(unit.source.len())
            .saturating_add(unit.path.as_os_str().as_encoded_bytes().len());
        unit.imports
            .iter()
            .chain(&unit.exports)
            .fold(total, |total, import| {
                total.saturating_add(import_payload(import))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    /// Diretório exclusivo dos testes, sem reutilizar arquivos existentes.
    struct Fixture(PathBuf);
    impl Fixture {
        /// Reserva um caminho novo mesmo após execução anterior interrompida.
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "dartforge-session-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("{error}"),
                }
            }
        }
        /// Grava fonte ou configuração somente no diretório reservado.
        fn write(&self, name: &str, source: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, source).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        /// Remove o diretório exclusivo; estes testes não criam links simbólicos.
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    /// Mesmo conteúdo reutiliza o Arc, mas o grafo continua sendo carregado.
    #[test]
    fn cache_hit_shares_output_and_reports_loaded_units() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'dep.dart'; void main(){print(f());}");
        fixture.write("dep.dart", "int f(){return 1;}");
        let mut session = CompilerSession::new();
        let first = session.compile_path(&entry, Optimization::None).unwrap();
        let second = session.compile_path(&entry, Optimization::None).unwrap();
        assert_eq!(
            first.stats,
            SessionStats {
                cache_hit: false,
                source_units_loaded: 2,
                compiled_units: 2,
                macro_plan_hits: 0,
                macro_plan_misses: 0,
            }
        );
        assert_eq!(
            second.stats,
            SessionStats {
                cache_hit: true,
                source_units_loaded: 2,
                compiled_units: 0,
                macro_plan_hits: 0,
                macro_plan_misses: 0,
            }
        );
        assert!(Arc::ptr_eq(&first.javascript, &second.javascript));
    }

    /// Alteração de mesmo tamanho e mtime preservado não pode servir saída antiga.
    #[test]
    fn dependency_change_is_detected_even_with_same_size_and_mtime() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'dep.dart'; void main(){print(f());}");
        let dep = fixture.write("dep.dart", "int f(){return 1;}");
        let metadata = std::fs::metadata(&dep).unwrap();
        let modified = metadata.modified().unwrap();
        let mut session = CompilerSession::new();
        let first = session.compile_path(&entry, Optimization::None).unwrap();
        fixture.write("dep.dart", "int f(){return 2;}");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&dep)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let after = std::fs::metadata(&dep).unwrap();
        assert_eq!(after.len(), metadata.len());
        assert_eq!(after.modified().unwrap(), modified);
        let changed = session.compile_path(&entry, Optimization::None).unwrap();
        assert!(!changed.stats.cache_hit);
        assert_ne!(first.javascript, changed.javascript);
    }

    /// Erros de leitura e análise limpam o cache; recuperação recompila o grafo.
    #[test]
    fn failures_never_return_previous_output_and_recovery_recompiles() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "import 'dep.dart'; void main(){print(f());}");
        let dep = fixture.write("dep.dart", "int f(){return 1;}");
        let mut session = CompilerSession::new();
        session.compile_path(&entry, Optimization::None).unwrap();
        std::fs::remove_file(&dep).unwrap();
        assert!(session.compile_path(&entry, Optimization::None).is_err());
        fixture.write("dep.dart", "int f(){return 1;}");
        assert!(
            !session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
        fixture.write("dep.dart", "int f(){return absent;}");
        assert!(session.compile_path(&entry, Optimization::None).is_err());
        fixture.write("dep.dart", "int f(){return 1;}");
        assert!(
            !session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
    }

    /// Troca da aresta ou opção recompila; só a última opção fica armazenada.
    #[test]
    fn changed_edges_and_optimization_invalidate_last_entry() {
        let fixture = Fixture::new();
        let entry = fixture.write(
            "main.dart",
            "import 'one.dart'; void main(){print(f()+2+3);}",
        );
        fixture.write("one.dart", "int f(){return 1;}");
        fixture.write("two.dart", "int f(){return 1;}");
        let mut session = CompilerSession::new();
        session.compile_path(&entry, Optimization::None).unwrap();
        fixture.write(
            "main.dart",
            "import 'two.dart'; void main(){print(f()+2+3);}",
        );
        assert!(
            !session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
        assert!(
            !session
                .compile_path(&entry, Optimization::Constants)
                .unwrap()
                .stats
                .cache_hit
        );
        assert!(
            session
                .compile_path(&entry, Optimization::Constants)
                .unwrap()
                .stats
                .cache_hit
        );
        assert!(
            !session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
    }

    /// Remapeamento de pacote invalida mesmo quando os dois arquivos são idênticos.
    #[test]
    fn package_config_remap_changes_canonical_graph_paths() {
        let fixture = Fixture::new();
        let entry = fixture.write(
            "main.dart",
            "import 'package:p/a.dart'; void main(){print(f());}",
        );
        fixture.write("v1/lib/a.dart", "int f(){return 1;}");
        fixture.write("v2/lib/a.dart", "int f(){return 1;}");
        let config = |version| {
            format!(
                r#"{{"configVersion":2,"packages":[{{"name":"p","rootUri":"../v{version}/","packageUri":"lib/","languageVersion":"3.6"}}]}}"#
            )
        };
        fixture.write(".dart_tool/package_config.json", &config(1));
        let mut session = CompilerSession::new();
        session.compile_path(&entry, Optimization::None).unwrap();
        assert!(
            session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
        fixture.write(".dart_tool/package_config.json", &config(2));
        assert!(
            !session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
    }

    /// Orçamento zero não retém resultados; clear invalida uma entrada existente.
    #[test]
    fn payload_budget_and_clear_bound_retention() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.dart", "void main(){}");
        let mut disabled = CompilerSession::with_cache_limit_bytes(0);
        for _ in 0..2 {
            assert!(
                !disabled
                    .compile_path(&entry, Optimization::None)
                    .unwrap()
                    .stats
                    .cache_hit
            );
        }
        let mut session = CompilerSession::new();
        session.compile_path(&entry, Optimization::None).unwrap();
        session.clear();
        assert!(
            !session
                .compile_path(&entry, Optimization::None)
                .unwrap()
                .stats
                .cache_hit
        );
    }
}
