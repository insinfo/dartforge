//! Carregador do fecho transitivo de bibliotecas Dart.
use crate::config::PackageConfig;
use crate::model::*;
use crate::outline;
use crate::sdk::SdkLayout;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DirectiveKind};
use dartforge_frontend::{Feature, LanguageVersion, LibraryFeatures};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use url::Url;

/// Representação intermediária desvinculada de empréstimos para processar diretivas.
enum DirectiveAction {
    Library(Vec<SymbolId>),
    /// `part 'uri'` ou, com `augmentation`, `import augment 'uri'` (forma
    /// 3.6 das bibliotecas de augmentation): a unidade apontada pertence a
    /// esta biblioteca.
    Part {
        uri: String,
        augmentation: bool,
    },
    Import {
        uri: String,
        prefix: Option<SymbolId>,
        deferred: bool,
        combinators: Vec<ast::Combinator>,
        span: Span,
    },
    Export {
        uri: String,
        combinators: Vec<ast::Combinator>,
        span: Span,
    },
}

/// Clona combinadores de uma diretiva de importação ou exportação.
fn clone_combinators(combinators: &[ast::Combinator]) -> Vec<ast::Combinator> {
    combinators
        .iter()
        .map(|c| match c {
            ast::Combinator::Show(names) => ast::Combinator::Show(names.clone()),
            ast::Combinator::Hide(names) => ast::Combinator::Hide(names.clone()),
        })
        .collect()
}

/// Extrai a string literal de um `ast::StringLit`.
pub fn string_lit_value(lit: &ast::StringLit) -> Option<String> {
    let mut out = String::new();
    for part in &lit.parts {
        match part {
            // URI com surrogate solto não é URI: `as_str` devolve `None`.
            ast::StringPart::Text(t) => out.push_str(t.as_str()?),
            _ => return None,
        }
    }
    Some(out)
}

/// Carrega um programa e acumula diagnósticos sem abortar, retornando o `Program` e `Vec<Diagnostic>`.
pub fn load_lenient(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_com_cache(entry, sdk, package_config_path, interner, None)
}

/// Como [`load_lenient`], tirando as unidades das bibliotecas `dart:` de
/// `cache` em vez de ler e analisar os arquivos do SDK.
///
/// O cache só é usado se os seus símbolos couberem em `interner` com os
/// mesmos ids (interner vazio, ou já preparado por este mesmo cache); caso
/// contrário a carga segue pelos arquivos.
pub fn load_lenient_com_cache(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_incremental(entry, sdk, package_config_path, interner, cache, None)
}

/// Como [`load_lenient_com_cache`], reaproveitando de `unidades` as unidades
/// cujos arquivos não mudaram (sessão residente).
///
/// O `interner` tem de ser o mesmo das análises que encheram o cache.
pub fn load_lenient_incremental(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
    unidades: Option<&mut crate::unidades::CacheUnidades>,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_gerados(entry, sdk, package_config_path, interner, cache, unidades, None)
}

/// Como `load_lenient_incremental`, com uma geração de fontes em memória
/// (`.template.dart` do ngdart e afins): o que estiver nela vale mais que o
/// disco e nem é lido de lá.
#[allow(clippy::too_many_arguments)]
pub fn load_lenient_gerados(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
    mut unidades: Option<&mut crate::unidades::CacheUnidades>,
    gerados: Option<std::sync::Arc<crate::gerado::Geracao>>,
) -> (Program, Vec<Diagnostic>) {
    if let Some(u) = unidades.as_deref_mut() {
        u.iniciar_carga();
    }
    let mut diagnostics = Vec::new();
    let mut cache = cache.filter(|c| c.preparar_interner(interner));
    // Bibliotecas cujas partes já vieram do cache (a diretiva `part` não
    // deve recarregá-las).
    let mut libs_do_cache: std::collections::HashSet<LibraryId> = std::collections::HashSet::new();

    // 1. Carrega o package_config.json
    let mut package_config = if let Some(p) = package_config_path {
        PackageConfig::load(p).unwrap_or_default()
    } else if let Some(discovered) = PackageConfig::discover(entry) {
        PackageConfig::load(&discovered).unwrap_or_default()
    } else {
        PackageConfig::default()
    };
    package_config.gerados = gerados;

    let mut program = Program::default();
    let mut uri_to_library: HashMap<String, LibraryId> = HashMap::new();
    let mut queue: VecDeque<LibraryId> = VecDeque::new();

    // 2. Garante o carregamento inicial de dart:core
    if sdk.library("core").is_some() {
        let core_uri = "dart:core".to_string();
        let core_id = LibraryId(program.libraries.len() as u32);
        program.libraries.push(Library {
            uri: core_uri.clone(),
            name: None,
            units: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            declared: HashMap::new(),
            exported: HashMap::new(),
            scope: HashMap::new(),
            prefixes: HashMap::new(),
            is_sdk: true,
            features: LibraryFeatures::piso(),
        });
        uri_to_library.insert(core_uri, core_id);
        program.core = Some(core_id);
        queue.push_back(core_id);
    }

    // 3. Registra e enfileira o ponto de entrada. É o único `canonicalize`
    // da carga (corrige maiúsculas e links do caminho dado pelo usuário);
    // tudo o que deriva dele — partes e imports relativos — é resolvido
    // lexicalmente, como a `Uri.resolve` do Dart faz.
    let canonical_entry = crate::config::sem_verbatim(
        std::fs::canonicalize(entry).unwrap_or_else(|_| entry.to_path_buf()),
    );
    let entry_uri = canonical_file_uri(&canonical_entry, &package_config);
    let mut prefetch = Prefetch { gerados: package_config.gerados.clone(), ..Default::default() };
    let mut considerados = 0usize;
    // Unidades do SDK já decodificadas do cache pela onda, à espera da vez da
    // biblioteca na fila.
    let mut sdk_decodificadas: HashMap<String, Vec<crate::sdk_cache::UnitCache>> = HashMap::new();
    let entry_lib_id = if let Some(&existing) = uri_to_library.get(&entry_uri) {
        existing
    } else {
        let lib_id = LibraryId(program.libraries.len() as u32);
        let is_sdk = entry_uri.starts_with("dart:");
        program.libraries.push(Library {
            uri: entry_uri.clone(),
            name: None,
            units: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            declared: HashMap::new(),
            exported: HashMap::new(),
            scope: HashMap::new(),
            prefixes: HashMap::new(),
            is_sdk,
            features: if is_sdk { LibraryFeatures::piso() } else { LibraryFeatures::atual() },
        });
        uri_to_library.insert(entry_uri.clone(), lib_id);
        queue.push_back(lib_id);
        lib_id
    };
    program.entry = Some(entry_lib_id);

    // 4. Processa a fila de bibliotecas transitivas
    while let Some(lib_id) = queue.pop_front() {
        // Onda: as bibliotecas já enfileiradas (um nível do grafo de imports)
        // são lidas e lexadas em paralelo antes de serem analisadas em série.
        // `considerados` evita reexaminar a fila inteira a cada iteração: as
        // bibliotecas ganham ids em ordem, então basta olhar as novas. (Sem
        // isso, uma compilação em que tudo vem do cache varria a fila 2.063
        // vezes — 11 s no `new_sali`.)
        if prefetch.vazio() && considerados < program.libraries.len() {
            let t = std::time::Instant::now();
            let mut caminhos: Vec<PathBuf> = Vec::new();
            for i in considerados..program.libraries.len() {
                let uri = &program.libraries[i].uri;
                if !uri.starts_with("dart:") {
                    if let Some(p) = caminho_da_biblioteca(uri, &package_config) {
                        // Unidade já analisada não precisa ser lida nem lexada
                        // aqui. Se o arquivo tiver mudado, `load_unit`
                        // descobre pela marca e lê ele mesmo — é um arquivo.
                        // (Conferir a marca aqui custava um `stat` por
                        // biblioteca da fila **em cada onda**: 34 mil no
                        // `new_sali`, 13 s de relógio no Windows.)
                        if !unidades.as_deref().is_some_and(|u| u.contem(&p)) {
                            caminhos.push(p);
                        }
                    }
                }
            }
            // As bibliotecas `dart:` da onda saem do cache decodificadas em
            // paralelo (em série, no perfil `dev`, custavam tanto quanto
            // reanalisar o SDK).
            if let Some(c) = cache.as_mut() {
                let t_cache = std::time::Instant::now();
                let nomes: Vec<&str> = program.libraries[considerados..]
                    .iter()
                    .filter_map(|l| l.uri.strip_prefix("dart:"))
                    .filter(|n| c.tem(n))
                    .collect();
                if !nomes.is_empty() {
                    sdk_decodificadas.extend(c.retirar_varios(&nomes));
                }
                program.tempos.sdk_cache += t_cache.elapsed();
            }
            considerados = program.libraries.len();
            prefetch.carregar(caminhos);
            program.tempos.leitura_lex_paralelo += t.elapsed();
            program.tempos.ondas += 1;
        }
        let lib_uri = program.libraries[lib_id.0 as usize].uri.clone();

        // Determina os arquivos da biblioteca (origem + patches se for SDK)
        if lib_uri.starts_with("dart:") {
            let lib_name = lib_uri.strip_prefix("dart:").unwrap();
            let t_cache = std::time::Instant::now();
            let do_cache = sdk_decodificadas
                .remove(lib_name)
                .or_else(|| cache.as_mut().and_then(|c| c.retirar(lib_name)));
            program.tempos.sdk_cache += t_cache.elapsed();
            if let Some(unidades) = do_cache {
                for u in unidades {
                    let unit_id = UnitId(program.units.len() as u32);
                    program.units.push(Unit {
                        uri: u.uri,
                        path: Some(u.path),
                        source: u.source,
                        ast: u.ast,
                        unit: u.unit,
                        library: lib_id,
                        role: u.role,
                        features: LibraryFeatures::piso(),
                    });
                    program.libraries[lib_id.0 as usize].units.push(unit_id);
                }
                libs_do_cache.insert(lib_id);
            } else if let Some(sdk_lib) = sdk.library(lib_name) {
                // Unidade principal
                let main_unit = load_unit(
                    &sdk_lib.path,
                    &lib_uri,
                    lib_id,
                    UnitRole::Library,
                    Versao::Sdk,
                    interner,
                    &mut program,
                    &mut diagnostics,
                    ler_substituto(sdk, &sdk_lib.path),
                    unidades.as_deref_mut(),
                );
                if let Some(uid) = main_unit {
                    program.libraries[lib_id.0 as usize].units.push(uid);
                }

                // Patches do SDK na ordem especificada em libraries.json
                for patch_path in &sdk_lib.patches {
                    let patch_uri = format!(
                        "{lib_uri}#patch_{}",
                        patch_path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    let patch_unit = load_unit(
                        patch_path,
                        &patch_uri,
                        lib_id,
                        UnitRole::Patch,
                        Versao::Sdk,
                        interner,
                        &mut program,
                        &mut diagnostics,
                        ler_substituto(sdk, patch_path),
                        unidades.as_deref_mut(),
                    );
                    if let Some(uid) = patch_unit {
                        program.libraries[lib_id.0 as usize].units.push(uid);
                    }
                }
            } else {
                diagnostics.push(Diagnostic::new(
                    format!("biblioteca SDK '{lib_uri}' não encontrada no layout"),
                    Span { start: 0, end: 0 },
                ));
            }
        } else {
            // Biblioteca de arquivo local ou pacote
            let file_path = caminho_da_biblioteca(&lib_uri, &package_config);

            if let Some(path) = file_path {
                let main_unit = load_unit(
                    &path,
                    &lib_uri,
                    lib_id,
                    UnitRole::Library,
                    Versao::Biblioteca { config: &package_config, corrente: sdk.versao_corrente, experimentos: &sdk.experimentos },
                    interner,
                    &mut program,
                    &mut diagnostics,
                    prefetch.tirar(&path),
                    unidades.as_deref_mut(),
                );
                if let Some(uid) = main_unit {
                    program.libraries[lib_id.0 as usize].units.push(uid);
                }
            } else {
                diagnostics.push(Diagnostic::new(
                    format!("não foi possível resolver caminho para '{lib_uri}'"),
                    Span { start: 0, end: 0 },
                ));
            }
        }

        // 5. Analisa diretivas em todas as unidades carregadas desta biblioteca
        let t_dir = std::time::Instant::now();
        let (leitura_antes, parse_antes) = (program.tempos.leitura, program.tempos.parse);
        let mut unit_idx = 0;
        while unit_idx < program.libraries[lib_id.0 as usize].units.len() {
            let unit_id = program.libraries[lib_id.0 as usize].units[unit_idx];
            unit_idx += 1;
            let unit_path = program.units[unit_id.0 as usize].path.clone();
            let unit_uri = program.units[unit_id.0 as usize].uri.clone();
            // Parte de um arquivo de patch (`core_patch.dart` tem
            // `part 'bigint_patch.dart'`) é patch também: as suas classes
            // `@patch` se fundem na classe de origem, em vez de criarem outra.
            let papel_das_partes = if program.units[unit_id.0 as usize].role == UnitRole::Patch {
                UnitRole::Patch
            } else {
                UnitRole::Part
            };

            // As unidades incluídas por esta (`part`, `import augment`) entram
            // logo depois dela, na ordem das diretivas: `Library::units` fica
            // na pré-ordem da árvore de partes, que é a ordem de aplicação
            // das augmentations (spec de augmentations, "Application order").
            let mut incluidas = 0usize;
            let actions: Vec<(usize, DirectiveAction)> = program.units[unit_id.0 as usize]
                .unit
                .directives
                .iter()
                .enumerate()
                .filter_map(|(dir_idx, directive)| match &directive.kind {
                    DirectiveKind::Library { name } => {
                        let syms = name.iter().map(|n| n.sym).collect();
                        Some((dir_idx, DirectiveAction::Library(syms)))
                    }
                    DirectiveKind::Part { uri } => {
                        let s = string_lit_value(uri)?;
                        Some((dir_idx, DirectiveAction::Part { uri: s, augmentation: false }))
                    }
                    DirectiveKind::ImportAugment { uri } => {
                        let s = string_lit_value(uri)?;
                        Some((dir_idx, DirectiveAction::Part { uri: s, augmentation: true }))
                    }
                    DirectiveKind::Import {
                        uri,
                        configurations,
                        deferred,
                        prefix,
                        combinators,
                    } => {
                        let selected = select_conditional_uri(uri, configurations, sdk, interner)?;
                        Some((
                            dir_idx,
                            DirectiveAction::Import {
                                uri: selected,
                                prefix: prefix.map(|n| n.sym),
                                deferred: *deferred,
                                combinators: clone_combinators(combinators),
                                span: directive.span,
                            },
                        ))
                    }
                    DirectiveKind::Export {
                        uri,
                        configurations,
                        combinators,
                    } => {
                        let selected = select_conditional_uri(uri, configurations, sdk, interner)?;
                        Some((
                            dir_idx,
                            DirectiveAction::Export {
                                uri: selected,
                                combinators: clone_combinators(combinators),
                                span: directive.span,
                            },
                        ))
                    }
                    _ => None,
                })
                .collect();

            for (dir_idx, action) in actions {
                match action {
                    DirectiveAction::Library(syms) => {
                        if program.libraries[lib_id.0 as usize].name.is_none() && !syms.is_empty() {
                            program.libraries[lib_id.0 as usize].name = Some(syms);
                        }
                    }
                    DirectiveAction::Part { uri, augmentation } => {
                        if libs_do_cache.contains(&lib_id) {
                            // Partes já vieram do cache, verificadas ao construí-lo.
                            continue;
                        }
                        let Some(base_path) = &unit_path else {
                            continue;
                        };
                        let pela_uri = resolver_relativo_a_package(&unit_uri, &uri)
                            .and_then(|u| package_config.resolve_package_uri(&u).ok().map(|p| (p, u)));
                        let (canonical_part, part_uri) = match pela_uri {
                            Some(x) => x,
                            None => {
                                let p = normalizar(&base_path.parent().unwrap_or(Path::new(".")).join(&uri));
                                let u = canonical_file_uri(&p, &package_config);
                                (p, u)
                            }
                        };
                        let da_biblioteca = program.libraries[lib_id.0 as usize].features;
                        let versao_da_parte = if program.libraries[lib_id.0 as usize].is_sdk {
                            Versao::Sdk
                        } else {
                            Versao::Parte { da_biblioteca, config: &package_config, corrente: sdk.versao_corrente }
                        };

                        let papel = if augmentation { UnitRole::Augmentation } else { papel_das_partes };
                        let part_unit = load_unit(
                            &canonical_part,
                            &part_uri,
                            lib_id,
                            papel,
                            versao_da_parte,
                            interner,
                            &mut program,
                            &mut diagnostics,
                            ler_substituto(sdk, &canonical_part).or_else(|| prefetch.tirar(&canonical_part)),
                            unidades.as_deref_mut(),
                        );

                        if let Some(p_uid) = part_unit {
                            if augmentation {
                                verify_augment_library(&program, p_uid, unit_path.as_deref(), &mut diagnostics);
                            } else {
                                verify_part_of(&program, p_uid, lib_id, &mut diagnostics);
                            }
                            // O SDK fica na ordem de descoberta de sempre (a
                            // mesma do cache do SDK): lá não há augmentation.
                            let libr = &mut program.libraries[lib_id.0 as usize];
                            if libr.is_sdk {
                                libr.units.push(p_uid);
                            } else {
                                libr.units.insert(unit_idx + incluidas, p_uid);
                                incluidas += 1;
                            }
                        }
                    }
                    DirectiveAction::Import {
                        uri,
                        prefix,
                        deferred,
                        combinators,
                        span,
                    } => {
                        let resolved = resolve_directive_target(
                            &uri,
                            &unit_uri,
                            unit_path.as_deref(),
                            sdk,
                            &package_config,
                            );

                        match resolved {
                            Ok((target_canonical_uri, _)) => {
                                let target_lib_id = get_or_create_library(
                                    &target_canonical_uri,
                                    &mut program,
                                    &mut uri_to_library,
                                    &mut queue,
                                );
                                program.libraries[lib_id.0 as usize].imports.push(Import {
                                    unit: unit_id,
                                    directive: dir_idx,
                                    library: target_lib_id,
                                    prefix,
                                    deferred,
                                    combinators,
                                });
                            }
                            Err(msg) => {
                                diagnostics.push(Diagnostic::new(msg, span));
                            }
                        }
                    }
                    DirectiveAction::Export {
                        uri,
                        combinators,
                        span,
                    } => {
                        let resolved = resolve_directive_target(
                            &uri,
                            &unit_uri,
                            unit_path.as_deref(),
                            sdk,
                            &package_config,
                            );

                        match resolved {
                            Ok((target_canonical_uri, _)) => {
                                let target_lib_id = get_or_create_library(
                                    &target_canonical_uri,
                                    &mut program,
                                    &mut uri_to_library,
                                    &mut queue,
                                );
                                program.libraries[lib_id.0 as usize].exports.push(Export {
                                    unit: unit_id,
                                    directive: dir_idx,
                                    library: target_lib_id,
                                    combinators,
                                });
                            }
                            Err(msg) => {
                                diagnostics.push(Diagnostic::new(msg, span));
                            }
                        }
                    }
                }
            }
        }
        // Partes carregadas aqui já contaram em leitura/parse.
        program.tempos.diretivas += t_dir
            .elapsed()
            .saturating_sub(program.tempos.leitura - leitura_antes)
            .saturating_sub(program.tempos.parse - parse_antes);
    }

    // 6. Constrói outline, namespaces e resolve supertipos
    outline::build_outline(&mut program, interner, &mut diagnostics);

    (program, diagnostics)
}

/// Carrega um programa a partir de um arquivo de entrada e seu SDK.
///
/// # Erros
/// Retorna `Err(Vec<Diagnostic>)` com todos os diagnósticos acumulados
/// de sintaxe, I/O ou resolução de diretivas.
pub fn load(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
) -> Result<Program, Vec<Diagnostic>> {
    let (program, diagnostics) = load_lenient(entry, sdk, package_config_path, interner);
    if diagnostics.is_empty() {
        Ok(program)
    } else {
        Err(diagnostics)
    }
}

// ---------------------------------------------------------------------------
// Funções auxiliares de resolução e carregamento
// ---------------------------------------------------------------------------

/// Fonte e tokens de um arquivo lido e lexado fora da thread principal.
type Lido = std::io::Result<(String, Result<Vec<dartforge_frontend::token::Token>, Diagnostic>)>;

/// Arquivos de uma onda da fila, lidos e lexados em paralelo.
#[derive(Default)]
struct Prefetch {
    prontos: HashMap<PathBuf, Lido>,
    /// Geração corrente: fontes daqui nunca vêm do disco.
    gerados: Option<std::sync::Arc<crate::gerado::Geracao>>,
}

impl Prefetch {
    fn vazio(&self) -> bool {
        self.prontos.is_empty()
    }

    fn tirar(&mut self, path: &Path) -> Option<Lido> {
        if let Some(f) = self.gerados.as_ref().and_then(|g| g.obter(path)) {
            let fonte = f.conteudo.to_string();
            let tokens = dartforge_frontend::lexer::lex(&fonte);
            return Some(Ok((fonte, tokens)));
        }
        self.prontos.remove(path)
    }

    /// Lê e lexa `caminhos` com até 8 threads (`std::thread::scope`); poucos
    /// arquivos ficam na própria thread, que o custo de criar threads não
    /// compensa.
    fn carregar(&mut self, caminhos: Vec<PathBuf>) {
        // O que a geração tem em memória não é lido do disco.
        let caminhos: Vec<PathBuf> = match &self.gerados {
            Some(g) => caminhos.into_iter().filter(|p| !g.contem(p)).collect(),
            None => caminhos,
        };
        let ler = |p: &Path| -> Lido {
            let s = std::fs::read_to_string(p)?;
            let t = dartforge_frontend::lexer::lex(&s);
            Ok((s, t))
        };
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        if caminhos.len() < 4 || threads < 2 {
            for p in caminhos {
                let l = ler(&p);
                self.prontos.insert(p, l);
            }
            return;
        }
        let proximo = std::sync::atomic::AtomicUsize::new(0);
        let resultados: Vec<Vec<(PathBuf, Lido)>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut meus = Vec::new();
                        loop {
                            let i = proximo.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(p) = caminhos.get(i) else { break };
                            let l = ler(p);
                            meus.push((p.clone(), l));
                        }
                        meus
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
        });
        for lote in resultados {
            for (p, l) in lote {
                self.prontos.insert(p, l);
            }
        }
    }
}

/// O texto de um arquivo do SDK que a sobreposição troca
/// (`SdkLayout::load_com_sobreposicao`): lido do substituto, com o caminho
/// lógico do original — é o que faz um `part` dele resolver ao lado do
/// original. `None` quando não há troca (a leitura segue o caminho normal).
fn ler_substituto(sdk: &SdkLayout, path: &Path) -> Option<Lido> {
    let novo = sdk.substituto(path)?;
    Some(std::fs::read_to_string(novo).map(|s| {
        let t = dartforge_frontend::lexer::lex(&s);
        (s, t)
    }))
}

/// Caminho do arquivo principal de uma biblioteca `package:` ou `file:`.
fn caminho_da_biblioteca(lib_uri: &str, package_config: &PackageConfig) -> Option<PathBuf> {
    if lib_uri.starts_with("package:") {
        package_config.resolve_package_uri(lib_uri).ok()
    } else if let Ok(url) = Url::parse(lib_uri) {
        url.to_file_path().ok()
    } else {
        Some(PathBuf::from(lib_uri))
    }
}

/// Resolve `.` e `..` sem tocar no sistema de arquivos (`Uri.resolve` do Dart
/// é lexical; `canonicalize` custava uma chamada ao sistema por diretiva).
pub(crate) fn normalizar(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// De onde vem a versão de linguagem de uma unidade
/// (`docs/VERSOES-LINGUAGEM.md` §2). Resolvida uma vez por unidade, antes do
/// parse, a partir da fonte já lida (o marcador) e do `package_config`.
#[derive(Clone, Copy)]
enum Versao<'c> {
    /// Biblioteca, patch ou parte do SDK: sempre o piso (D1).
    Sdk,
    /// Unidade principal de uma biblioteca do usuário ou de pacote.
    Biblioteca { config: &'c PackageConfig, corrente: LanguageVersion, experimentos: &'c [Feature] },
    /// Parte: analisada com os recursos da biblioteca; a versão da própria
    /// parte (marcador ou pacote) tem de ser a mesma, senão é erro
    /// (`LanguageVersionMismatchInPart`).
    Parte { da_biblioteca: LibraryFeatures, config: &'c PackageConfig, corrente: LanguageVersion },
}

/// Os recursos de uma unidade e os diagnósticos da escolha da versão.
fn features_da_unidade(
    fonte: &str,
    uri: &str,
    path: &Path,
    versao: Versao<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> LibraryFeatures {
    match versao {
        Versao::Sdk => LibraryFeatures::piso(),
        Versao::Biblioteca { config, corrente, experimentos } => {
            let (padrao, marcador) = versao_propria(fonte, uri, path, config, corrente, diagnostics);
            LibraryFeatures::para_biblioteca(marcador.map(|m| m.0), padrao, corrente, experimentos)
        }
        Versao::Parte { da_biblioteca, config, corrente } => {
            let (padrao, marcador) = versao_propria(fonte, uri, path, config, corrente, diagnostics);
            let propria = marcador.map_or(padrao, |m| m.0);
            if propria != da_biblioteca.versao() {
                let span = marcador.map_or(Span { start: 0, end: 0 }, |m| m.1);
                diagnostics.push(Diagnostic::new(
                    format!(
                        "{}:{}: a parte está na versão de linguagem {propria}, mas a biblioteca dela está na {}: uma parte tem de ter a versão da sua biblioteca",
                        path.display(),
                        span.start,
                        da_biblioteca.versao()
                    ),
                    span,
                ));
            }
            da_biblioteca
        }
    }
}

/// A versão padrão da unidade (a do pacote, ou a corrente fora de pacote) e
/// o marcador `// @dart = x.y` válido, com o seu intervalo. Marcador acima da
/// corrente ou abaixo de 2.12 é erro e fica de fora, como no CFE
/// (`LanguageVersionTooHighExplicit`/`TooLowExplicit`).
fn versao_propria(
    fonte: &str,
    uri: &str,
    path: &Path,
    config: &PackageConfig,
    corrente: LanguageVersion,
    diagnostics: &mut Vec<Diagnostic>,
) -> (LanguageVersion, Option<(LanguageVersion, Span)>) {
    let mut erro = |msg: String, span: Span| {
        diagnostics.push(Diagnostic::new(format!("{}:{}: {msg}", path.display(), span.start), span));
    };
    let zero = Span { start: 0, end: 0 };
    let padrao = match config.pacote_da_biblioteca(uri, Some(path)) {
        Some(p) => {
            if let Some(t) = &p.language_version_invalida {
                erro(format!("o languageVersion '{t}' do pacote '{}' no package_config.json não é uma versão x.y", p.name), zero);
            }
            match p.language_version {
                Some(v) if v > corrente => {
                    erro(format!("o pacote '{}' está na versão de linguagem {v}, acima da suportada ({corrente})", p.name), zero);
                    corrente
                }
                Some(v) => v,
                None => corrente,
            }
        }
        None => corrente,
    };
    let marcador = dartforge_frontend::features::marcador_versao(fonte).and_then(|(v, span)| {
        if v > corrente {
            erro(format!("a versão de linguagem {v} do marcador está acima da suportada ({corrente})"), span);
            None
        } else if v < LanguageVersion::MINIMA {
            erro(
                format!("a versão de linguagem {v} do marcador está abaixo da mínima ({}, null safety)", LanguageVersion::MINIMA),
                span,
            );
            None
        } else {
            Some((v, span))
        }
    });
    (padrao, marcador)
}

#[allow(clippy::too_many_arguments)]
fn load_unit(
    path: &Path,
    uri: &str,
    lib_id: LibraryId,
    role: UnitRole,
    versao: Versao<'_>,
    interner: &mut Interner,
    program: &mut Program,
    diagnostics: &mut Vec<Diagnostic>,
    lido: Option<Lido>,
    unidades: Option<&mut crate::unidades::CacheUnidades>,
) -> Option<UnitId> {
    // Sessão residente: unidade já analisada entra direto (o que mudou já foi
    // invalidado pelo dono do cache; a carga não consulta o disco por isso).
    // A versão é recalculada da fonte guardada (o marcador) e do
    // `package_config` de agora: se o `languageVersion` do pacote mudou, a
    // unidade é analisada de novo.
    let mut unidades = unidades;
    if let Some(cache) = unidades.as_deref_mut() {
        let tirada = cache.tirar(path);
        if let Some(mut u) = tirada {
            let mut diags_versao = Vec::new();
            let features = features_da_unidade(&u.source, uri, path, versao, &mut diags_versao);
            let ok = crate::unidades::serve(&u, uri, role) && u.features == features;
            if ok {
                diagnostics.extend(diags_versao);
                if role == UnitRole::Library {
                    program.libraries[lib_id.0 as usize].features = features;
                }
                u.library = lib_id;
                let unit_id = UnitId(program.units.len() as u32);
                program.units.push(u);
                program.tempos.unidades_reaproveitadas += 1;
                return Some(unit_id);
            }
        }
    }
    let t = std::time::Instant::now();
    let lido = match lido {
        Some(l) => l,
        None => std::fs::read_to_string(path).map(|s| {
            let t = dartforge_frontend::lexer::lex(&s);
            (s, t)
        }),
    };
    let (source, tokens) = match lido {
        Ok(x) => x,
        Err(e) => {
            diagnostics.push(Diagnostic::new(
                format!("não foi possível ler {}: {e}", path.display()),
                Span { start: 0, end: 0 },
            ));
            return None;
        }
    };
    program.tempos.leitura += t.elapsed();
    program.tempos.arquivos_lidos += 1;
    program.tempos.bytes_lidos += source.len();
    // Um único `stat` por arquivo lido, para a sessão saber quando ele mudar.
    if let Some(cache) = unidades.as_deref_mut() {
        cache.anotar_marca(path);
    }

    let features = features_da_unidade(&source, uri, path, versao, diagnostics);
    if role == UnitRole::Library {
        program.libraries[lib_id.0 as usize].features = features;
    }
    let t = std::time::Instant::now();
    let parsed = dartforge_frontend::parser::parse_lexed_com(&source, tokens, interner, features);
    program.tempos.parse += t.elapsed();
    for mut d in parsed.diagnostics {
        d.message = format!("{}:{}: {}", path.display(), d.span.start, d.message);
        diagnostics.push(d);
    }

    let unit_id = UnitId(program.units.len() as u32);
    program.units.push(Unit {
        uri: uri.to_string(),
        path: Some(path.to_path_buf()),
        source,
        ast: parsed.ast,
        unit: parsed.unit,
        library: lib_id,
        role,
        features,
    });

    Some(unit_id)
}

fn get_or_create_library(
    canonical_uri: &str,
    program: &mut Program,
    uri_to_library: &mut HashMap<String, LibraryId>,
    queue: &mut VecDeque<LibraryId>,
) -> LibraryId {
    if let Some(&existing) = uri_to_library.get(canonical_uri) {
        existing
    } else {
        let lib_id = LibraryId(program.libraries.len() as u32);
        let is_sdk = canonical_uri.starts_with("dart:");
        program.libraries.push(Library {
            uri: canonical_uri.to_string(),
            name: None,
            units: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            declared: HashMap::new(),
            exported: HashMap::new(),
            scope: HashMap::new(),
            prefixes: HashMap::new(),
            is_sdk,
            features: if is_sdk { LibraryFeatures::piso() } else { LibraryFeatures::atual() },
        });
        uri_to_library.insert(canonical_uri.to_string(), lib_id);
        queue.push_back(lib_id);
        lib_id
    }
}

/// URI canônica de um caminho **já canonizado** pelo chamador: `package:x/y.dart`
/// quando está dentro do `packageUri` de um pacote ou dos gerados do
/// `build_runner` (`<projeto>/.dart_tool/build/generated/<pacote>/lib/<rel>`),
/// senão `file:///…`.
///
/// Não chama `canonicalize`: os diretórios dos pacotes e a raiz de gerados
/// vêm prontos de [`PackageConfig`] (canonizados uma vez na carga) e o
/// caminho chega canônico (entrada, parte ou import relativo, cada um
/// canonizado uma vez por quem o descobriu).
fn canonical_file_uri(canonical: &Path, package_config: &PackageConfig) -> String {
    // No Windows `canonicalize` devolve `\?\C:\…`; os diretórios dos pacotes
    // estão sem o prefixo, então o caminho também fica sem ele.
    let canonical = crate::config::sem_verbatim(canonical.to_path_buf());
    if let Some(gen_root) = &package_config.generated_root {
        if let Ok(rel) = canonical.strip_prefix(gen_root) {
            let parts: Vec<String> = rel.iter().map(|c| c.to_string_lossy().into_owned()).collect();
            if parts.len() >= 3 && parts[1] == "lib" {
                return format!("package:{}/{}", parts[0], parts[2..].join("/"));
            }
        }
    }
    for (name, dir) in &package_config.package_dirs {
        if let Ok(rel) = canonical.strip_prefix(dir) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            return format!("package:{name}/{rel_str}");
        }
    }
    Url::from_file_path(&canonical)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| canonical.to_string_lossy().to_string())
}

/// Resolve `rel` (sem esquema) contra `base` (`package:x/a/b.dart`) só com
/// texto, como `Uri.resolve`: `None` se `..` sair do pacote (aí o chamador
/// resolve pelo caminho) ou se `rel` tiver esquema.
fn resolver_relativo_a_package(base: &str, rel: &str) -> Option<String> {
    if rel.contains(':') || rel.starts_with('/') {
        return None;
    }
    let corpo = base.strip_prefix("package:")?;
    let (pacote, resto) = corpo.split_once('/')?;
    let mut segs: Vec<&str> = resto.split('/').collect();
    segs.pop(); // o arquivo base
    for s in rel.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop()?;
            }
            s => segs.push(s),
        }
    }
    let mut out = String::with_capacity(base.len() + rel.len());
    out.push_str("package:");
    out.push_str(pacote);
    for s in segs {
        out.push('/');
        out.push_str(s);
    }
    Some(out)
}

fn resolve_directive_target(
    uri_str: &str,
    base_uri: &str,
    base_path: Option<&Path>,
    sdk: &SdkLayout,
    package_config: &PackageConfig,
) -> Result<(String, Option<PathBuf>), String> {
    if let Some(sdk_name) = uri_str.strip_prefix("dart:") {
        if sdk.library(sdk_name).is_some() {
            Ok((format!("dart:{sdk_name}"), None))
        } else {
            Err(format!("biblioteca SDK 'dart:{sdk_name}' não encontrada"))
        }
    } else if uri_str.starts_with("package:") {
        // Só valida o pacote; o caminho é calculado uma vez, quando a
        // biblioteca sai da fila (`caminho_da_biblioteca`), não por diretiva.
        let nome = uri_str["package:".len()..].split('/').next().unwrap_or("");
        if package_config.packages.contains_key(nome) {
            Ok((uri_str.to_string(), None))
        } else {
            let file_path = package_config.resolve_package_uri(uri_str)?;
            Ok((uri_str.to_string(), Some(file_path)))
        }
    } else if let Some(uri) = resolver_relativo_a_package(base_uri, uri_str) {
        Ok((uri, None))
    } else {
        // Relativo ao arquivo base. Se o base está num pacote (real ou gerado),
        // resolve pela URI `package:` para que gerados e originais se encontrem.
        if let Some(bp) = base_path {
            let base_uri = canonical_file_uri(bp, package_config);
            if let Some(rest) = base_uri.strip_prefix("package:") {
                // `package:` não é hierárquica para o `url`: junta-se à mão.
                let mut segs: Vec<&str> = rest.split('/').collect();
                segs.pop();
                for part in uri_str.split('/') {
                    match part {
                        "." | "" => {}
                        ".." => {
                            segs.pop();
                        }
                        p => segs.push(p),
                    }
                }
                if segs.len() >= 2 {
                    let joined = format!("package:{}", segs.join("/"));
                    if let Ok(file_path) = package_config.resolve_package_uri(&joined) {
                        if file_path.is_file() {
                            return Ok((joined, Some(file_path)));
                        }
                    }
                }
            }
        }
        let base = base_path.and_then(|p| p.parent()).unwrap_or(Path::new("."));
        let target_path = base.join(uri_str);
        // Lexical, como `Uri.resolve`; arquivo inexistente falha na leitura.
        let canonical = normalizar(&target_path);
        let canonical_uri = canonical_file_uri(&canonical, package_config);
        Ok((canonical_uri, Some(canonical)))
    }
}

fn select_conditional_uri(
    primary: &ast::StringLit,
    configurations: &[ast::Configuration],
    sdk: &SdkLayout,
    interner: &Interner,
) -> Option<String> {
    for config in configurations {
        if evaluate_configuration(config, sdk, interner) {
            return string_lit_value(&config.uri);
        }
    }
    string_lit_value(primary)
}

fn evaluate_configuration(
    config: &ast::Configuration,
    sdk: &SdkLayout,
    interner: &Interner,
) -> bool {
    // Avalia expressões do tipo: dart.library.io == 'true' ou dart.library.io
    if config.test.len() == 3
        && interner.resolve(config.test[0].sym) == "dart"
        && interner.resolve(config.test[1].sym) == "library"
    {
        let lib_name = interner.resolve(config.test[2].sym);
        let supported = sdk.library(lib_name).is_some_and(|l| l.supported);
        if let Some(expected_val) = &config.value {
            if let Some(val_str) = string_lit_value(expected_val) {
                return (val_str == "true") == supported;
            }
        }
        return supported;
    }
    false
}

/// A unidade de `import augment 'x'` tem de começar por `augment library 'y'`
/// com `y` apontando para quem a importou (a regra do par `part`/`part of`).
fn verify_augment_library(
    program: &Program,
    aug_unit_id: UnitId,
    quem_importa: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let unidade = &program.units[aug_unit_id.0 as usize];
    let cabecalho = unidade.unit.directives.iter().find_map(|d| match &d.kind {
        DirectiveKind::AugmentLibrary { uri } => Some((d.span, uri)),
        _ => None,
    });
    let Some((span, uri)) = cabecalho else {
        diagnostics.push(Diagnostic::new(
            format!("a biblioteca de augmentation '{}' não começa por 'augment library'", unidade.uri),
            Span { start: 0, end: 0 },
        ));
        return;
    };
    let (Some(alvo), Some(caminho), Some(importador)) = (string_lit_value(uri), &unidade.path, quem_importa) else {
        return;
    };
    let esperado = normalizar(&caminho.parent().unwrap_or(Path::new(".")).join(&alvo));
    if esperado != normalizar(importador) {
        diagnostics.push(Diagnostic::new(
            format!(
                "'augment library' aponta para '{}', mas quem importa a augmentation é '{}'",
                esperado.display(),
                importador.display()
            ),
            span,
        ));
    }
}

fn verify_part_of(
    program: &Program,
    part_unit_id: UnitId,
    lib_id: LibraryId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let part_unit = &program.units[part_unit_id.0 as usize];
    let parent_lib = &program.libraries[lib_id.0 as usize];
    let mut found_part_of = false;

    for d in &part_unit.unit.directives {
        if let DirectiveKind::PartOf { uri, name } = &d.kind {
            found_part_of = true;
            if let Some(parent_uri_lit) = uri {
                if let Some(parent_uri_str) = string_lit_value(parent_uri_lit) {
                    if let Some(part_path) = &part_unit.path {
                        let expected_parent = part_path
                            .parent()
                            .unwrap_or(Path::new("."))
                            .join(&parent_uri_str);
                        let canonical_expected = normalizar(&expected_parent);
                        let matches_any_unit = parent_lib.units.iter().any(|&u| {
                            if let Some(p) = &program.units[u.0 as usize].path {
                                normalizar(p) == canonical_expected
                            } else {
                                false
                            }
                        });
                        if !matches_any_unit {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "diretiva 'part of' aponta para '{}', mas a biblioteca declarante é '{}'",
                                    expected_parent.display(),
                                    parent_lib.uri
                                ),
                                d.span,
                            ));
                        }
                    }
                }
            } else if !name.is_empty() {
                let part_name_syms: Vec<_> = name.iter().map(|n| n.sym).collect();
                if parent_lib.name.as_ref() != Some(&part_name_syms) {
                    diagnostics.push(Diagnostic::new(
                        "nome da biblioteca na diretiva 'part of' não corresponde à biblioteca declarante",
                        d.span,
                    ));
                }
            }
        }
    }

    if !found_part_of {
        diagnostics.push(Diagnostic::new(
            format!(
                "arquivo de parte '{}' não contém diretiva 'part of'",
                part_unit.uri
            ),
            Span { start: 0, end: 0 },
        ));
    }
}
