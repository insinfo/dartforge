//! Loader of the transitive closure of Dart libraries.
use crate::config::PackageConfig;
use crate::model::*;
use crate::outline;
use crate::sdk::SdkLayout;
use dartforge_diagnostics::{Diagnostic, Span, codigos};
use dartforge_frontend::ast::{self, DirectiveKind};
use dartforge_frontend::{Feature, LanguageVersion, LibraryFeatures};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use url::Url;

/// Intermediate representation free of borrows, for processing directives.
enum DirectiveAction {
    Library(Vec<SymbolId>),
    /// `part 'uri'` or, with `augmentation`, `import augment 'uri'` (the 3.6
    /// form of augmentation libraries): the unit pointed to belongs to this
    /// library.
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

/// Clones the combinators of an import or export directive.
fn clone_combinators(combinators: &[ast::Combinator]) -> Vec<ast::Combinator> {
    combinators
        .iter()
        .map(|c| match c {
            ast::Combinator::Show(names) => ast::Combinator::Show(names.clone()),
            ast::Combinator::Hide(names) => ast::Combinator::Hide(names.clone()),
        })
        .collect()
}

/// Extracts the string literal of an `ast::StringLit`.
pub fn string_lit_value(lit: &ast::StringLit) -> Option<String> {
    let mut out = String::new();
    for part in &lit.parts {
        match part {
            // A URI with a lone surrogate is not a URI: `as_str` returns `None`.
            ast::StringPart::Text(t) => out.push_str(t.as_str()?),
            _ => return None,
        }
    }
    Some(out)
}

/// Loads a program and accumulates diagnostics without aborting, returning
/// the `Program` and a `Vec<Diagnostic>`.
pub fn load_lenient(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_com_cache(entry, sdk, package_config_path, interner, None)
}

/// Like [`load_lenient`], taking the units of the `dart:` libraries from
/// `cache` instead of reading and parsing the SDK files.
///
/// The cache is only used if its symbols fit in `interner` with the same
/// ids (an empty interner, or one already prepared by this same cache);
/// otherwise the load goes through the files.
pub fn load_lenient_com_cache(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_incremental(entry, sdk, package_config_path, interner, cache, None)
}

/// Like [`load_lenient_com_cache`], reusing from `unidades` the units whose
/// files did not change (resident session).
///
/// The `interner` must be the same as that of the analyses that filled the
/// cache.
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

/// Like `load_lenient_incremental`, with an in-memory source generation
/// (ngdart's `.template.dart` and the like): what is in it wins over the
/// disk and is not even read from there.
#[allow(clippy::too_many_arguments)]
pub fn load_lenient_gerados(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
    unidades: Option<&mut crate::unidades::CacheUnidades>,
    gerados: Option<std::sync::Arc<crate::gerado::Geracao>>,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_entradas(&[entry], sdk, package_config_path, interner, cache, unidades, gerados)
}

/// Like [`load_lenient_gerados`], with several entry libraries in the same
/// program: the first one is [`Program::entry`] and the others only go into
/// the queue, with what they import. It is the load of a whole project (the
/// LSP's rename needs to see the libraries that import the declaration, not
/// only those it imports). The entries must be libraries, not parts
/// (`part of`): a part loaded as a library would appear twice.
///
/// # Panics
///
/// When `entries` is empty.
#[allow(clippy::too_many_arguments)]
pub fn load_lenient_entradas(
    entries: &[&Path],
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
    unidades: Option<&mut crate::unidades::CacheUnidades>,
    gerados: Option<std::sync::Arc<crate::gerado::Geracao>>,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_(entries, sdk, package_config_path, interner, cache, unidades, gerados, None)
}

/// Like [`load_lenient`], without seeing `ocultos` (keys from
/// [`crate::gerado::chave`]): the load a `build_runner` builder makes of the
/// program, without the outputs of the phases that have not run for it yet.
pub fn load_lenient_hiding(
    entry: &Path,
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    ocultos: std::sync::Arc<std::collections::HashSet<PathBuf>>,
) -> (Program, Vec<Diagnostic>) {
    load_lenient_(&[entry], sdk, package_config_path, interner, None, None, None, Some(ocultos))
}

/// Like [`load_lenient_hiding`], with several roots: the first one is the
/// program's entry point; the others go into the queue as libraries. A root
/// that is a part (`part of`) is dropped — it comes in through its owning
/// library, and loading it as a library too would duplicate it.
pub fn load_lenient_entradas_ocultando(
    entries: &[&Path],
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    ocultos: std::sync::Arc<std::collections::HashSet<PathBuf>>,
) -> (Program, Vec<Diagnostic>) {
    let mut roots: Vec<&Path> = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if i == 0 || !is_part_on_disk(e, interner) {
            roots.push(e);
        }
    }
    load_lenient_(&roots, sdk, package_config_path, interner, None, None, None, Some(ocultos))
}

/// Does the file have the `part of` directive? Unreadable counts as no.
fn is_part_on_disk(caminho: &Path, interner: &mut Interner) -> bool {
    let Ok(text) = std::fs::read_to_string(caminho) else {
        return false;
    };
    let tokens = dartforge_frontend::lexer::lex(&text);
    let parsed_unit = dartforge_frontend::parser::parse_lexed(&text, tokens, interner);
    parsed_unit
        .unit
        .directives
        .iter()
        .any(|d| matches!(d.kind, DirectiveKind::PartOf { .. }))
}

#[allow(clippy::too_many_arguments)]
fn load_lenient_(
    entries: &[&Path],
    sdk: &SdkLayout,
    package_config_path: Option<&Path>,
    interner: &mut Interner,
    cache: Option<crate::sdk_cache::SdkCache>,
    mut unidades: Option<&mut crate::unidades::CacheUnidades>,
    gerados: Option<std::sync::Arc<crate::gerado::Geracao>>,
    ocultos: Option<std::sync::Arc<std::collections::HashSet<PathBuf>>>,
) -> (Program, Vec<Diagnostic>) {
    let entry = *entries.first().expect("at least one entry");
    if let Some(u) = unidades.as_deref_mut() {
        u.start_load();
    }
    let mut diagnostics = Vec::new();
    let mut cache = cache.filter(|c| c.prepare_interner(interner));
    // Libraries whose parts already came from the cache (the `part` directive
    // must not reload them).
    let mut cached_libs: std::collections::HashSet<LibraryId> = std::collections::HashSet::new();

    // 1. Loads package_config.json
    let mut package_config = if let Some(p) = package_config_path {
        PackageConfig::load(p).unwrap_or_default()
    } else if let Some(discovered) = PackageConfig::discover(entry) {
        PackageConfig::load(&discovered).unwrap_or_default()
    } else {
        PackageConfig::default()
    };
    package_config.gerados = gerados;
    package_config.ocultos = ocultos;

    let mut program = Program::default();
    let mut uri_to_library: HashMap<String, LibraryId> = HashMap::new();
    let mut queue: VecDeque<LibraryId> = VecDeque::new();

    // 2. Makes sure dart:core is loaded first
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
            pais: HashMap::new(),
            representante: Default::default(),
            dono_do_grupo: Default::default(),
            escopos_de_unidade: HashMap::new(),
            is_sdk: true,
            features: LibraryFeatures::piso(),
        });
        uri_to_library.insert(core_uri, core_id);
        program.core = Some(core_id);
        queue.push_back(core_id);
    }

    // 3. Registers and enqueues the entry points. It is the load's only
    // `canonicalize` (it fixes the case and links of the path the user gave);
    // everything derived from it — parts and relative imports — is resolved
    // lexically, as Dart's `Uri.resolve` does.
    let mut prefetch = Prefetch {
        gerados: package_config.gerados.clone(),
        ocultos: package_config.ocultos.clone(),
        ..Default::default()
    };
    let mut considered = 0usize;
    // SDK units already decoded from the cache by the wave, waiting for their
    // library's turn in the queue.
    let mut decoded_sdk: HashMap<String, Vec<crate::sdk_cache::UnitCache>> = HashMap::new();
    for (i, entry_path) in entries.iter().enumerate() {
        let canonical_entry = crate::config::sem_verbatim(
            std::fs::canonicalize(entry_path).unwrap_or_else(|_| entry_path.to_path_buf()),
        );
        let entry_uri = canonical_file_uri(&canonical_entry, &package_config);
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
                pais: HashMap::new(),
                representante: Default::default(),
                dono_do_grupo: Default::default(),
                escopos_de_unidade: HashMap::new(),
                is_sdk,
                features: if is_sdk { LibraryFeatures::piso() } else { LibraryFeatures::atual() },
            });
            uri_to_library.insert(entry_uri.clone(), lib_id);
            queue.push_back(lib_id);
            lib_id
        };
        if i == 0 {
            program.entry = Some(entry_lib_id);
        }
    }

    // 4. Processes the queue of transitive libraries
    while let Some(lib_id) = queue.pop_front() {
        // Wave: the libraries already enqueued (one level of the import graph) are
        // read and lexed in parallel before being parsed serially. `considered`
        // avoids reexamining the whole queue on every iteration: libraries get ids
        // in order, so it is enough to look at the new ones. (Without this, a
        // compilation where everything comes from the cache swept the queue 2,063
        // times — 11 s in `new_sali`.)
        if prefetch.empty() && considered < program.libraries.len() {
            let t = std::time::Instant::now();
            let mut caminhos: Vec<PathBuf> = Vec::new();
            for i in considered..program.libraries.len() {
                let uri = &program.libraries[i].uri;
                if !uri.starts_with("dart:") {
                    if let Some(p) = library_path(uri, &package_config) {
                        // An already analyzed unit does not need to be read or lexed here. If the
                        // file changed, `load_unit` finds out through the stamp and reads it
                        // itself — it is one file. (Checking the stamp here cost one `stat` per
                        // queued library **on every wave**: 34 thousand in `new_sali`, 13 s of wall
                        // clock on Windows.)
                        if !unidades.as_deref().is_some_and(|u| u.contem(&p)) {
                            caminhos.push(p);
                        }
                    }
                }
            }
            // The wave's `dart:` libraries come out of the cache decoded in parallel
            // (serially, in the `dev` profile, they cost as much as reanalyzing the
            // SDK).
            if let Some(c) = cache.as_mut() {
                let t_cache = std::time::Instant::now();
                let names_: Vec<&str> = program.libraries[considered..]
                    .iter()
                    .filter_map(|l| l.uri.strip_prefix("dart:"))
                    .filter(|n| c.tem(n))
                    .collect();
                if !names_.is_empty() {
                    decoded_sdk.extend(c.remove_several(&names_));
                }
                program.tempos.sdk_cache += t_cache.elapsed();
            }
            considered = program.libraries.len();
            prefetch.load_(caminhos);
            program.tempos.leitura_lex_paralelo += t.elapsed();
            program.tempos.ondas += 1;
        }
        let lib_uri = program.libraries[lib_id.0 as usize].uri.clone();

        // Determines the library's files (source + patches if it is the SDK)
        if lib_uri.starts_with("dart:") {
            let lib_name = lib_uri.strip_prefix("dart:").unwrap();
            let t_cache = std::time::Instant::now();
            let from_cache = decoded_sdk
                .remove(lib_name)
                .or_else(|| cache.as_mut().and_then(|c| c.retirar(lib_name)));
            program.tempos.sdk_cache += t_cache.elapsed();
            if let Some(unidades) = from_cache {
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
                        pulados: Vec::new(),
                        referencia: dartforge_diagnostics::Referencia::V3_6,
                    });
                    program.libraries[lib_id.0 as usize].units.push(unit_id);
                }
                cached_libs.insert(lib_id);
            } else if let Some(sdk_lib) = sdk.library(lib_name) {
                // Main unit
                let main_unit = load_unit(
                    &sdk_lib.path,
                    &lib_uri,
                    lib_id,
                    UnitRole::Library,
                    Version::Sdk,
                    interner,
                    &mut program,
                    &mut diagnostics,
                    read_substitute(sdk, &sdk_lib.path),
                    unidades.as_deref_mut(),
                );
                if let Some(uid) = main_unit {
                    program.libraries[lib_id.0 as usize].units.push(uid);
                }

                // SDK patches in the order given in libraries.json
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
                        Version::Sdk,
                        interner,
                        &mut program,
                        &mut diagnostics,
                        read_substitute(sdk, patch_path),
                        unidades.as_deref_mut(),
                    );
                    if let Some(uid) = patch_unit {
                        program.libraries[lib_id.0 as usize].units.push(uid);
                    }
                }
            } else {
                diagnostics.push(Diagnostic::new(
                    format!("SDK library '{lib_uri}' not found in the layout"),
                    Span { start: 0, end: 0 },
                ));
            }
        } else {
            // A local file or package library
            let file_path = library_path(&lib_uri, &package_config);

            if let Some(path) = file_path {
                let main_unit = load_unit(
                    &path,
                    &lib_uri,
                    lib_id,
                    UnitRole::Library,
                    Version::Library { config: &package_config, current_version: sdk.versao_corrente, experimentos: &sdk.experimentos },
                    interner,
                    &mut program,
                    &mut diagnostics,
                    prefetch.take(&path),
                    unidades.as_deref_mut(),
                );
                if let Some(uid) = main_unit {
                    program.libraries[lib_id.0 as usize].units.push(uid);
                }
            } else {
                diagnostics.push(Diagnostic::new(
                    format!("could not resolve a path for '{lib_uri}'"),
                    Span { start: 0, end: 0 },
                ));
            }
        }

        // 5. Analyzes the directives in every loaded unit of this library
        let t_dir = std::time::Instant::now();
        let (read_before, parse_before) = (program.tempos.leitura, program.tempos.parse);
        let mut unit_idx = 0;
        // The augmentation the macro host built for this library (in memory,
        // `<name>.macro.dart` next to it) comes in last, after every part — like
        // the augmentation libraries the CFE creates for macro output. It is only
        // consulted when there are generated sources (zero cost without macros).
        let mut attached_macro = false;
        loop {
            if unit_idx >= program.libraries[lib_id.0 as usize].units.len() {
                if attached_macro {
                    break;
                }
                attached_macro = true;
                match attach_macro_augmentation(&mut program, lib_id, &package_config, sdk.versao_corrente, interner, &mut diagnostics, &mut prefetch) {
                    Some(uid) => {
                        program.libraries[lib_id.0 as usize].units.push(uid);
                        continue;
                    }
                    None => break,
                }
            }
            let unit_id = program.libraries[lib_id.0 as usize].units[unit_idx];
            unit_idx += 1;
            let unit_path = program.units[unit_id.0 as usize].path.clone();
            let unit_uri = program.units[unit_id.0 as usize].uri.clone();
            // A part of a patch file (`core_patch.dart` has
            // `part 'bigint_patch.dart'`) is a patch too: its `@patch` classes merge
            // into the source class, instead of creating another one.
            let part_roles = if program.units[unit_id.0 as usize].role == UnitRole::Patch {
                UnitRole::Patch
            } else {
                UnitRole::Part
            };

            // The units included by this one (`part`, `import augment`) come right
            // after it, in directive order: `Library::units` stays in the preorder of
            // the parts tree, which is the application order of the augmentations
            // (augmentations spec, "Application order").
            let mut included = 0usize;
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
                        if cached_libs.contains(&lib_id) {
                            // Parts already came from the cache, checked when building it.
                            continue;
                        }
                        let Some(base_path) = &unit_path else {
                            continue;
                        };
                        // `part 'package:p/x.dart'` (`diff_match_patch`): the URI is already
                        // absolute; the rest is relative to the library's.
                        let absolute = uri.starts_with("package:").then(|| uri.clone());
                        let by_uri = absolute
                            .or_else(|| resolve_relative_to_package(&unit_uri, &uri))
                            .and_then(|u| package_config.resolve_package_uri(&u).ok().map(|p| (p, u)));
                        let (canonical_part, part_uri) = match by_uri {
                            Some(x) => x,
                            None => {
                                let p = normalize(&base_path.parent().unwrap_or(Path::new(".")).join(&uri));
                                let u = canonical_file_uri(&p, &package_config);
                                (p, u)
                            }
                        };
                        // A part that is already a unit of this library (a `part` of the file
                        // itself, or a cycle of nested parts) is not loaded again: the load would
                        // not end.
                        let already_in_library = program.libraries[lib_id.0 as usize]
                            .units
                            .iter()
                            .copied()
                            .find(|u| program.units[u.0 as usize].path.as_deref() == Some(canonical_part.as_path()));
                        if already_in_library.is_some() {
                            continue;
                        }
                        let of_library = program.libraries[lib_id.0 as usize].features;
                        let part_version = if program.libraries[lib_id.0 as usize].is_sdk {
                            Version::Sdk
                        } else {
                            Version::Part { of_library, config: &package_config, current_version: sdk.versao_corrente }
                        };

                        let role_ = if augmentation { UnitRole::Augmentation } else { part_roles };
                        let part_unit = load_unit(
                            &canonical_part,
                            &part_uri,
                            lib_id,
                            role_,
                            part_version,
                            interner,
                            &mut program,
                            &mut diagnostics,
                            read_substitute(sdk, &canonical_part).or_else(|| prefetch.take(&canonical_part)),
                            unidades.as_deref_mut(),
                        );

                        if let Some(p_uid) = part_unit {
                            if augmentation {
                                verify_augment_library(&program, p_uid, unit_path.as_deref(), &mut diagnostics);
                            } else {
                                verify_part_of(&program, p_uid, lib_id, &mut diagnostics);
                            }
                            // The SDK stays in the usual discovery order (the same as the SDK cache's):
                            // there are no augmentations there.
                            let libr = &mut program.libraries[lib_id.0 as usize];
                            libr.pais.insert(p_uid, unit_id);
                            if libr.is_sdk {
                                libr.units.push(p_uid);
                            } else {
                                libr.units.insert(unit_idx + included, p_uid);
                                included += 1;
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
        // Parts loaded here were already counted in read/parse.
        program.tempos.diretivas += t_dir
            .elapsed()
            .saturating_sub(program.tempos.leitura - read_before)
            .saturating_sub(program.tempos.parse - parse_before);
    }

    // 6. Builds the outline and namespaces and resolves supertypes
    outline::build_outline(&mut program, interner, &mut diagnostics);

    (program, diagnostics)
}

/// Loads a program from an entry file and its SDK.
///
/// # Errors
/// Returns `Err(Vec<Diagnostic>)` with all the accumulated syntax, I/O or
/// directive resolution diagnostics.
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
// Resolution and loading helpers
// ---------------------------------------------------------------------------

/// Source and tokens of a file read and lexed off the main thread.
type ReadFile = std::io::Result<(String, Result<Vec<dartforge_frontend::token::Token>, Diagnostic>)>;

/// Files of one queue wave, read and lexed in parallel.
#[derive(Default)]
struct Prefetch {
    ready: HashMap<PathBuf, ReadFile>,
    /// Current generation: sources from here never come from the disk.
    gerados: Option<std::sync::Arc<crate::gerado::Geracao>>,
    /// What the load does not see ([`PackageConfig::ocultos`]): read as a
    /// nonexistent file.
    ocultos: Option<std::sync::Arc<std::collections::HashSet<PathBuf>>>,
}

impl Prefetch {
    fn empty(&self) -> bool {
        self.ready.is_empty()
    }

    fn take(&mut self, path: &Path) -> Option<ReadFile> {
        if let Some(f) = self.gerados.as_ref().and_then(|g| g.obter(path)) {
            let source = f.conteudo.to_string();
            let tokens = dartforge_frontend::lexer::lex(&source);
            return Some(Ok((source, tokens)));
        }
        self.ready.remove(path)
    }

    /// Reads and lexes `caminhos` with up to 8 threads (`std::thread::scope`);
    /// a few files stay on the current thread, since the cost of creating
    /// threads does not pay off.
    fn load_(&mut self, caminhos: Vec<PathBuf>) {
        // What the generation has in memory is not read from the disk.
        let caminhos: Vec<PathBuf> = match &self.gerados {
            Some(g) => caminhos.into_iter().filter(|p| !g.contem(p)).collect(),
            None => caminhos,
        };
        let (ocultos, caminhos): (Vec<PathBuf>, Vec<PathBuf>) = caminhos.into_iter().partition(|p| {
            self.ocultos.as_ref().is_some_and(|o| o.contains(&crate::gerado::chave(p)))
        });
        for p in ocultos {
            self.ready.insert(p, Err(std::io::ErrorKind::NotFound.into()));
        }
        let ler = |p: &Path| -> ReadFile {
            let s = std::fs::read_to_string(p)?;
            let t = dartforge_frontend::lexer::lex(&s);
            Ok((s, t))
        };
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        if caminhos.len() < 4 || threads < 2 {
            for p in caminhos {
                let l = ler(&p);
                self.ready.insert(p, l);
            }
            return;
        }
        let next_index = std::sync::atomic::AtomicUsize::new(0);
        let results: Vec<Vec<(PathBuf, ReadFile)>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    s.spawn(|| {
                        let mut mine = Vec::new();
                        loop {
                            let i = next_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(p) = caminhos.get(i) else { break };
                            let l = ler(p);
                            mine.push((p.clone(), l));
                        }
                        mine
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
        });
        for lote in results {
            for (p, l) in lote {
                self.ready.insert(p, l);
            }
        }
    }
}

/// The text of an SDK file the overlay swaps
/// (`SdkLayout::load_com_sobreposicao`): read from the substitute, with the
/// original's logical path — that is what makes a `part` of it resolve next
/// to the original. `None` when there is no swap (reading follows the normal
/// path).
fn read_substitute(sdk: &SdkLayout, path: &Path) -> Option<ReadFile> {
    let new_ = sdk.substituto(path)?;
    Some(std::fs::read_to_string(new_).map(|s| {
        let t = dartforge_frontend::lexer::lex(&s);
        (s, t)
    }))
}

/// Path of the main file of a `package:` or `file:` library.
fn library_path(lib_uri: &str, package_config: &PackageConfig) -> Option<PathBuf> {
    if lib_uri.starts_with("package:") {
        package_config.resolve_package_uri(lib_uri).ok()
    } else if let Ok(url) = Url::parse(lib_uri) {
        let p = url.to_file_path().ok()?;
        // `file:` without a file in place: the `build_runner` generated file, if
        // any.
        if package_config.generated_root.is_some()
            && !p.is_file()
            && !package_config.gerados.as_ref().is_some_and(|g| g.contem(&p))
        {
            if let Some(gerado) = package_config.gerado_no_lugar_de(&p) {
                return Some(gerado);
            }
        }
        Some(p)
    } else {
        Some(PathBuf::from(lib_uri))
    }
}

/// Resolves `.` and `..` without touching the file system (Dart's
/// `Uri.resolve` is lexical; `canonicalize` cost a system call per
/// directive).
pub(crate) fn normalize(p: &Path) -> PathBuf {
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

/// Where a unit's language version comes from (`docs/VERSOES-LINGUAGEM.md`
/// §2). Resolved once per unit, before parsing, from the source already read
/// (the marker) and the `package_config`.
#[derive(Clone, Copy)]
enum Version<'c> {
    /// SDK library, patch or part: always the floor (D1).
    Sdk,
    /// Main unit of a user or package library.
    Library { config: &'c PackageConfig, current_version: LanguageVersion, experimentos: &'c [Feature] },
    /// Part: parsed with the library's features; the part's own version (marker
    /// or package) must be the same, otherwise it is an error
    /// (`LanguageVersionMismatchInPart`).
    Part { of_library: LibraryFeatures, config: &'c PackageConfig, current_version: LanguageVersion },
}

/// The features of a unit and the diagnostics of the version choice.
fn unit_features(
    source: &str,
    uri: &str,
    path: &Path,
    version: Version<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> LibraryFeatures {
    match version {
        Version::Sdk => LibraryFeatures::piso(),
        Version::Library { config, current_version, experimentos } => {
            let (default_version, marker) = own_version(source, uri, path, config, current_version, diagnostics);
            LibraryFeatures::for_library(marker.map(|m| m.0), default_version, current_version, experimentos)
        }
        Version::Part { of_library, config, current_version } => {
            let (default_version, marker) = own_version(source, uri, path, config, current_version, diagnostics);
            let own = marker.map_or(default_version, |m| m.0);
            if own != of_library.versao() {
                let span = marker.map_or(Span { start: 0, end: 0 }, |m| m.1);
                diagnostics.push(Diagnostic::new(
                    format!(
                        "{}:{}: the part is at language version {own}, but its library is at {}: a part must have the version of its library",
                        path.display(),
                        span.start,
                        of_library.versao()
                    ),
                    span,
                ));
            }
            of_library
        }
    }
}

/// The unit's default version (the package's, or the current one outside a
/// package) and the valid `// @dart = x.y` marker, with its range. A marker
/// above the current version or below 2.12 is an error and is left out, as
/// in the CFE (`LanguageVersionTooHighExplicit`/`TooLowExplicit`).
fn own_version(
    source: &str,
    uri: &str,
    path: &Path,
    config: &PackageConfig,
    current_version: LanguageVersion,
    diagnostics: &mut Vec<Diagnostic>,
) -> (LanguageVersion, Option<(LanguageVersion, Span)>) {
    // The path and offset in front of the message, like the parser's
    // diagnostics; the code (when the analyzer has one) stays in the
    // diagnostic.
    let prefix = |span: Span| format!("{}:{}: ", path.display(), span.start);
    let mut error_code = |mut d: Diagnostic| {
        d.message = format!("{}{}", prefix(d.span), d.message);
        diagnostics.push(d);
    };
    let zero = Span { start: 0, end: 0 };
    let default_version = match config.pacote_da_biblioteca(uri, Some(path)) {
        Some(p) => {
            if let Some(t) = &p.invalid_language_version {
                error_code(Diagnostic::new(format!("The languageVersion '{t}' of package '{}' in package_config.json isn't a valid x.y version.", p.name), zero));
            }
            match p.language_version {
                Some(v) if v > current_version => {
                    error_code(Diagnostic::new(format!("The package '{}' has language version {v}, greater than the latest known language version: {current_version}.", p.name), zero));
                    current_version
                }
                Some(v) => v,
                None => current_version,
            }
        }
        None => current_version,
    };
    let marker = dartforge_frontend::features::marcador_versao(source).and_then(|(v, span)| {
        if v > current_version {
            // WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER.
            let (greater, lesser) = (current_version.major.to_string(), current_version.minor.to_string());
            error_code(Diagnostic::com_codigo(
                codigos::warning::INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER,
                span,
                [greater.as_str(), lesser.as_str()],
            ));
            None
        } else if v < LanguageVersion::MINIMA {
            // CompileTimeErrorCode.ILLEGAL_LANGUAGE_VERSION_OVERRIDE.
            let minima = format!(">={}.0", LanguageVersion::MINIMA);
            error_code(Diagnostic::com_codigo(codigos::compile_time_error::ILLEGAL_LANGUAGE_VERSION_OVERRIDE, span, [minima.as_str()]));
            None
        } else {
            Some((v, span))
        }
    });
    (default_version, marker)
}

#[allow(clippy::too_many_arguments)]
fn load_unit(
    path: &Path,
    uri: &str,
    lib_id: LibraryId,
    role: UnitRole,
    version: Version<'_>,
    interner: &mut Interner,
    program: &mut Program,
    diagnostics: &mut Vec<Diagnostic>,
    read_file: Option<ReadFile>,
    unidades: Option<&mut crate::unidades::CacheUnidades>,
) -> Option<UnitId> {
    // Resident session: an already analyzed unit comes straight in (what
    // changed was already invalidated by the cache's owner; the load does not
    // consult the disk for that). The version is recomputed from the stored
    // source (the marker) and the current `package_config`: if the package's
    // `languageVersion` changed, the unit is analyzed again.
    let mut unidades = unidades;
    if let Some(cache) = unidades.as_deref_mut() {
        let removed = cache.take(path);
        if let Some(mut u) = removed {
            let mut version_diags = Vec::new();
            let features = unit_features(&u.source, uri, path, version, &mut version_diags);
            let ok = crate::unidades::serve(&u, uri, role) && u.features == features;
            if ok {
                diagnostics.extend(version_diags);
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
    let read_file = match read_file {
        Some(l) => l,
        None => std::fs::read_to_string(path).map(|s| {
            let t = dartforge_frontend::lexer::lex(&s);
            (s, t)
        }),
    };
    let (source, tokens) = match read_file {
        Ok(x) => x,
        Err(e) => {
            diagnostics.push(Diagnostic::new(
                format!("could not read {}: {e}", path.display()),
                Span { start: 0, end: 0 },
            ));
            return None;
        }
    };
    program.tempos.leitura += t.elapsed();
    program.tempos.arquivos_lidos += 1;
    program.tempos.bytes_lidos += source.len();
    // A single `stat` per file read, so the session knows when it changes.
    if let Some(cache) = unidades.as_deref_mut() {
        cache.annotate_marker(path);
    }

    let features = unit_features(&source, uri, path, version, diagnostics);
    if role == UnitRole::Library {
        program.libraries[lib_id.0 as usize].features = features;
    }
    let t = std::time::Instant::now();
    let parsed = dartforge_frontend::parser::parse_lexed_with(&source, tokens, interner, features);
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
        pulados: parsed.pulados,
        referencia: parsed.referencia,
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
            pais: HashMap::new(),
            representante: Default::default(),
            dono_do_grupo: Default::default(),
            escopos_de_unidade: HashMap::new(),
            is_sdk,
            features: if is_sdk { LibraryFeatures::piso() } else { LibraryFeatures::atual() },
        });
        uri_to_library.insert(canonical_uri.to_string(), lib_id);
        queue.push_back(lib_id);
        lib_id
    }
}

/// Canonical URI of a path **already canonicalized** by the caller:
/// `package:x/y.dart` when it is inside a package's `packageUri` or the
/// `build_runner` generated files
/// (`<project>/.dart_tool/build/generated/<package>/lib/<rel>`), otherwise
/// `file:///…`.
///
/// Does not call `canonicalize`: the package directories and the generated
/// root come ready from [`PackageConfig`] (canonicalized once at load time)
/// and the path arrives canonical (entry, part or relative import, each one
/// canonicalized once by whoever discovered it).
fn canonical_file_uri(canonical: &Path, package_config: &PackageConfig) -> String {
    // On Windows `canonicalize` returns `\?\C:\…`; the package directories are
    // without the prefix, so the path is too.
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

/// Resolves `rel` (without a scheme) against `base` (`package:x/a/b.dart`)
/// with text only, like `Uri.resolve`: `None` if `..` leaves the package
/// (then the caller resolves through the path) or if `rel` has a scheme.
fn resolve_relative_to_package(base: &str, rel: &str) -> Option<String> {
    if rel.contains(':') || rel.starts_with("//") {
        return None;
    }
    let body_node = base.strip_prefix("package:")?;
    let (package, rest) = body_node.split_once('/')?;
    let mut segs: Vec<&str> = rest.split('/').collect();
    segs.pop(); // the base file
    // `/x.dart` in a `package:` library: the package root (the VM and the
    // 3.6.2 analyzer resolve `package:p/src/a.dart` + `/x.dart` as
    // `package:p/x.dart`; checked with `dart run` and `dart analyze`).
    if rel.starts_with('/') {
        segs.clear();
    }
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
    out.push_str(package);
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
    // `import '' as self;` (`sqlite3`'s generated `native.dart`): the empty URI,
    // resolved against the library's, is the library itself (RFC 3986 §5.2: an
    // empty reference = the base). Joined to the directory, it gave "could not
    // read …/generated: Is a directory".
    if uri_str.is_empty() {
        return Ok((base_uri.to_string(), base_path.map(Path::to_path_buf)));
    }
    if let Some(sdk_name) = uri_str.strip_prefix("dart:") {
        if sdk.library(sdk_name).is_some() {
            Ok((format!("dart:{sdk_name}"), None))
        } else {
            // Like the CFE: a library of another platform (`dart:js_interop` on native,
            // `dart:io` without the VM layout) is a compile error.
            Err(format!("the Dart library 'dart:{sdk_name}' is not available on this platform"))
        }
    } else if uri_str.starts_with("package:") {
        // Only validates the package; the path is computed once, when the library
        // leaves the queue (`library_path`), not per directive.
        let name_ = uri_str["package:".len()..].split('/').next().unwrap_or("");
        if package_config.packages.contains_key(name_) {
            Ok((uri_str.to_string(), None))
        } else {
            let file_path = package_config.resolve_package_uri(uri_str)?;
            Ok((uri_str.to_string(), Some(file_path)))
        }
    } else if let Some(uri) = resolve_relative_to_package(base_uri, uri_str) {
        Ok((uri, None))
    } else {
        // Relative to the base file. If the base is in a package (real or
        // generated), resolves through the `package:` URI so that generated and
        // original files find each other.
        if let Some(bp) = base_path {
            let base_uri = canonical_file_uri(bp, package_config);
            if let Some(rest) = base_uri.strip_prefix("package:") {
                // `package:` is not hierarchical for `url`: joined by hand.
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
        // A `build_runner` generated file resolves relative imports from its
        // original place (`test/x.template.dart` imports `x.dart` from `test/`).
        let origin = base_path.and_then(|p| package_config.generated_origin(p));
        let base = origin.as_deref().or(base_path).and_then(|p| p.parent()).unwrap_or(Path::new("."));
        let target_path = base.join(uri_str);
        // Lexical, like `Uri.resolve`; a nonexistent file fails when read.
        let canonical = normalize(&target_path);
        let canonical_uri = canonical_file_uri(&canonical, package_config);
        // Outside `lib/` (`test/`, `web/`), the `build_runner` generated file takes
        // the place of the missing file, with the place's URI.
        if !canonical.is_file() && !package_config.gerados.as_ref().is_some_and(|g| g.contem(&canonical)) {
            if let Some(gerado) = package_config.gerado_no_lugar_de(&canonical) {
                return Ok((canonical_uri, Some(gerado)));
            }
        }
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
    // Evaluates expressions like: dart.library.io == 'true' or dart.library.io
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

/// The path of a library's macro augmentation: `x.macro.dart` next to
/// `x.dart` (the same name as the materialization,
/// docs/MACROS-COMPATIBILIDADE.md).
pub fn caminho_da_augmentation_de_macro(library: &Path) -> PathBuf {
    let stem = library.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    library.with_file_name(format!("{stem}.macro.dart"))
}

/// Loads the macro augmentation of `lib`, if the in-memory generation has
/// one (and the library does not include it yet through
/// `import augment`/`part`).
fn attach_macro_augmentation(
    program: &mut Program,
    lib_id: LibraryId,
    package_config: &PackageConfig,
    current_version: LanguageVersion,
    interner: &mut Interner,
    diagnostics: &mut Vec<Diagnostic>,
    prefetch: &mut Prefetch,
) -> Option<UnitId> {
    let gerados = package_config.gerados.as_ref().filter(|g| !g.vazia())?;
    let lib = &program.libraries[lib_id.0 as usize];
    if lib.is_sdk {
        return None;
    }
    let principal = program.units[lib.units.first()?.0 as usize].path.clone()?;
    let caminho = caminho_da_augmentation_de_macro(&principal);
    if !gerados.contem(&caminho) {
        return None;
    }
    let chave = crate::gerado::chave(&caminho);
    if lib.units.iter().any(|u| program.units[u.0 as usize].path.as_deref().map(crate::gerado::chave).as_ref() == Some(&chave)) {
        return None;
    }
    let uri = canonical_file_uri(&caminho, package_config);
    let version = Version::Part { of_library: lib.features, config: package_config, current_version };
    let read_file = prefetch.take(&caminho);
    let uid = load_unit(&caminho, &uri, lib_id, UnitRole::Augmentation, version, interner, program, diagnostics, read_file, None)?;
    Some(uid)
}

/// The unit of `import augment 'x'` must start with `augment library 'y'`
/// with `y` pointing to whoever imported it (the rule of the
/// `part`/`part of` pair).
fn verify_augment_library(
    program: &Program,
    aug_unit_id: UnitId,
    importers: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let unidade = &program.units[aug_unit_id.0 as usize];
    let header = unidade.unit.directives.iter().find_map(|d| match &d.kind {
        DirectiveKind::AugmentLibrary { uri } => Some((d.span, uri)),
        _ => None,
    });
    let Some((span, uri)) = header else {
        diagnostics.push(Diagnostic::new(
            format!("augmentation library '{}' does not start with 'augment library'", unidade.uri),
            Span { start: 0, end: 0 },
        ));
        return;
    };
    let (Some(target), Some(caminho), Some(importer)) = (string_lit_value(uri), &unidade.path, importers) else {
        return;
    };
    let expected = normalize(&caminho.parent().unwrap_or(Path::new(".")).join(&target));
    if expected != normalize(importer) {
        diagnostics.push(Diagnostic::new(
            format!(
                "'augment library' points to '{}', but the library importing the augmentation is '{}'",
                expected.display(),
                importer.display()
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
                        let canonical_expected = normalize(&expected_parent);
                        let matches_any_unit = parent_lib.units.iter().any(|&u| {
                            if let Some(p) = &program.units[u.0 as usize].path {
                                normalize(p) == canonical_expected
                            } else {
                                false
                            }
                        });
                        if !matches_any_unit {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "'part of' directive points to '{}', but the declaring library is '{}'",
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
                        "library name in the 'part of' directive does not match the declaring library",
                        d.span,
                    ));
                }
            }
        }
    }

    if !found_part_of {
        diagnostics.push(Diagnostic::new(
            format!(
                "part file '{}' does not contain a 'part of' directive",
                part_unit.uri
            ),
            Span { start: 0, end: 0 },
        ));
    }
}
