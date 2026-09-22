//! Carregador do fecho transitivo de bibliotecas Dart.
use crate::config::PackageConfig;
use crate::model::*;
use crate::outline;
use crate::sdk::SdkLayout;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DirectiveKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use url::Url;

/// Representação intermediária desvinculada de empréstimos para processar diretivas.
enum DirectiveAction {
    Library(Vec<SymbolId>),
    Part {
        uri: String,
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

/// Carrega um programa e acumula diagnósticos sem abortar, retornando o Program e Vec<Diagnostic>.
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
    let mut diagnostics = Vec::new();
    let mut cache = cache.filter(|c| c.preparar_interner(interner));
    // Bibliotecas cujas partes já vieram do cache (a diretiva `part` não
    // deve recarregá-las).
    let mut libs_do_cache: std::collections::HashSet<LibraryId> = std::collections::HashSet::new();

    // 1. Carrega o package_config.json
    let package_config = if let Some(p) = package_config_path {
        PackageConfig::load(p).unwrap_or_default()
    } else if let Some(discovered) = PackageConfig::discover(entry) {
        PackageConfig::load(&discovered).unwrap_or_default()
    } else {
        PackageConfig::default()
    };

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
            language_version: None,
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
    let mut prefetch = Prefetch::default();
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
            language_version: None,
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
        if prefetch.vazio() {
            let t = std::time::Instant::now();
            let mut caminhos: Vec<PathBuf> = Vec::new();
            for id in std::iter::once(&lib_id).chain(queue.iter()) {
                let uri = &program.libraries[id.0 as usize].uri;
                if !uri.starts_with("dart:") {
                    if let Some(p) = caminho_da_biblioteca(uri, &package_config) {
                        caminhos.push(p);
                    }
                }
            }
            prefetch.carregar(caminhos);
            program.tempos.leitura_lex_paralelo += t.elapsed();
            program.tempos.ondas += 1;
        }
        let lib_uri = program.libraries[lib_id.0 as usize].uri.clone();

        // Determina os arquivos da biblioteca (origem + patches se for SDK)
        if lib_uri.starts_with("dart:") {
            let lib_name = lib_uri.strip_prefix("dart:").unwrap();
            let t_cache = std::time::Instant::now();
            let do_cache = cache.as_mut().and_then(|c| c.retirar(lib_name));
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
                    interner,
                    &mut program,
                    &mut diagnostics,
                    None,
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
                        interner,
                        &mut program,
                        &mut diagnostics,
                        None,
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
                    interner,
                    &mut program,
                    &mut diagnostics,
                    prefetch.tirar(&path),
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
                        Some((dir_idx, DirectiveAction::Part { uri: s }))
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
                    DirectiveAction::Part { uri } => {
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

                        let part_unit = load_unit(
                            &canonical_part,
                            &part_uri,
                            lib_id,
                            UnitRole::Part,
                            interner,
                            &mut program,
                            &mut diagnostics,
                            None,
                        );

                        if let Some(p_uid) = part_unit {
                            verify_part_of(&program, p_uid, lib_id, &mut diagnostics);
                            program.libraries[lib_id.0 as usize].units.push(p_uid);
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
}

impl Prefetch {
    fn vazio(&self) -> bool {
        self.prontos.is_empty()
    }

    fn tirar(&mut self, path: &Path) -> Option<Lido> {
        self.prontos.remove(path)
    }

    /// Lê e lexa `caminhos` com até 8 threads (`std::thread::scope`); poucos
    /// arquivos ficam na própria thread, que o custo de criar threads não
    /// compensa.
    fn carregar(&mut self, caminhos: Vec<PathBuf>) {
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
fn normalizar(p: &Path) -> PathBuf {
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

fn load_unit(
    path: &Path,
    uri: &str,
    lib_id: LibraryId,
    role: UnitRole,
    interner: &mut Interner,
    program: &mut Program,
    diagnostics: &mut Vec<Diagnostic>,
    lido: Option<Lido>,
) -> Option<UnitId> {
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

    let t = std::time::Instant::now();
    let parsed = dartforge_frontend::parser::parse_lexed(&source, tokens, interner);
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
            language_version: None,
        });
        uri_to_library.insert(canonical_uri.to_string(), lib_id);
        queue.push_back(lib_id);
        lib_id
    }
}

/// URI canônica de um caminho **já canonizado** pelo chamador: `package:x/y.dart`
/// quando está dentro do `packageUri` de um pacote, senão `file:///…`.
///
/// Não chama `canonicalize`: os diretórios dos pacotes vêm prontos de
/// [`PackageConfig::package_dirs`] e o caminho chega canônico (entrada, parte
/// ou import relativo, cada um canonizado uma vez por quem o descobriu).
fn canonical_file_uri(canonical: &Path, package_config: &PackageConfig) -> String {
    // No Windows `canonicalize` devolve `\\?\C:\…`; os diretórios dos pacotes
    // estão sem o prefixo, então o caminho também fica sem ele.
    let canonical = crate::config::sem_verbatim(canonical.to_path_buf());
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
        // Relativo ao arquivo base
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
