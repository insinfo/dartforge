//! `ngdart:ngdart` through `ng_generator`, stage A (`docs/BUILD-MOTOR.md`;
//! engine plan §6.2): **one package action**, over the session's `Program`
//! (the `Resolvedor` is `BuildStep.resolver` without extra loading), with
//! conservative queries — any `.dart`/`.html`/`.scss`/`.css` of the package,
//! the file list of `lib/`, `web/` and `test/`, and the text of every library
//! outside the package that the package's libraries reach. The engine cuts
//! off by output: only the `.template.dart`/`.css.shim.dart` with new text
//! invalidate units. It applies to the input's package and to every
//! dependency ngdart applies to, each with its own round.
//!
//! Stage B, inside the package action: an edit to an already known HTML file
//! or stylesheet regenerates only the components that read it; an edit to a
//! `.dart` of the package regenerates that file and whoever reaches it
//! through `import`/`export` (`try_dart`). The rest (new or deleted file,
//! part, refusal, another package) follows stage A; the package is still the
//! engine's revalidation unit.
use crate::query::Query;
use crate::executor::{GeneratorCtx, NativeGenerator, NativeRequest, NativeOutput};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct NgStageA {
    /// The builder key: `ngdart:ngdart` (ngdart 8) or `ngx_dart:ngx_dart` (the
    /// version 9 fork with `package:web`); `ng_generator` picks the dialect
    /// from the program (`docs/NGDART-COMPILADOR-DE-VISOES.md` part B).
    key: &'static str,
    /// Per package: the input and each dependency with components have their
    /// own round.
    cache: Mutex<HashMap<String, NgCache>>,
}

impl Default for NgStageA {
    fn default() -> Self {
        Self::with_key("ngdart:ngdart")
    }
}

impl NgStageA {
    /// The generator for the builder `key` (`ngdart:ngdart` or
    /// `ngx_dart:ngx_dart`).
    pub fn with_key(key: &'static str) -> Self {
        NgStageA {
            key,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

struct NgCache {
    output: NativeOutput,
    /// HTML/CSS/SCSS -> Dart sources that read the resource.
    resource_readers: BTreeMap<PathBuf, Vec<PathBuf>>,
    index: Option<dartforge_ng_generator::Indice>,
}

/// Something in the text that may be Angular (conservative: any mention of
/// ngdart, of the annotations or of an injector).
fn looks_angular(text: &str) -> bool {
    const MARKERS: &[&str] = &[
        "ngdart",
        "angular",
        "@Component",
        "@Directive",
        "@Pipe",
        "@Injectable",
        "GenerateInjector",
        "Injector",
        "@Input",
        "@Output",
        "@HostBinding",
        "@HostListener",
        "@View",
        "@Content",
        "OpaqueToken",
        "Provider",
    ];
    MARKERS.iter().any(|m| text.contains(m))
}

const EXTENSIONS: &[&str] = &["dart", "html", "scss", "sass", "css"];

/// The `.css` stylesheets that `sass_builder` (or another `SassBuilder`) of
/// this build generated for the package's `.scss`/`.sass` files (in the
/// engine's memory): ngdart's shim starts from them, as in the official
/// builder, with the project's `outputStyle`. A `.css` stylesheet on disk
/// stays with the disk.
fn generated_sheets(ctx: &mut GeneratorCtx<'_>, root: &Path) -> HashMap<PathBuf, String> {
    let mut sources = Vec::new();
    for d in ["lib", "web", "test"] {
        collect_files(&root.join(d), &mut sources);
    }
    let mut sheets = HashMap::new();
    for scss in sources {
        let partial = scss
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('_'));
        if partial || scss.extension().is_none_or(|e| e != "scss" && e != "sass") {
            continue;
        }
        // `x.css` from `sass_builder`, or `x.scss.css` from a `SassBuilder`
        // with `outputExtension: '.scss.css'` (ngcomponents' `scss_builder`).
        let mut scss_css_name = scss.clone().into_os_string();
        scss_css_name.push(".css");
        for css in [scss.with_extension("css"), PathBuf::from(scss_css_name)] {
            if css.is_file() {
                continue;
            }
            if let Some(b) = ctx.read(&css) {
                sheets.insert(css, String::from_utf8_lossy(&b).into_owned());
            }
        }
    }
    sheets
}

fn collect_files(dir: &std::path::Path, v: &mut Vec<PathBuf>) {
    let Ok(ls) = std::fs::read_dir(dir) else {
        return;
    };
    let mut es: Vec<_> = ls.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            collect_files(&p, v);
        } else if p
            .extension()
            .is_some_and(|x| EXTENSIONS.iter().any(|e| x == *e))
        {
            v.push(p);
        }
    }
}

impl NativeGenerator for NgStageA {
    fn key(&self) -> &'static str {
        self.key
    }

    fn covers(&self, factory: &str) -> bool {
        factory == "templateCompiler" || factory == "stylesheetCompiler"
    }

    fn by_package(&self) -> bool {
        true
    }

    fn verified(&self) -> bool {
        // Verified byte for byte: 0 differences against the oracle of the
        // ngdart corpus and of new_sali (ESTADO §1.7); what it does not know,
        // it refuses.
        true
    }

    fn generate(
        &self,
        ctx: &mut GeneratorCtx<'_>,
        request: &NativeRequest,
    ) -> Result<NativeOutput, String> {
        let root = &request.package_root;
        // Any package ngdart applies to: the input's package and the
        // dependencies with components (a `path` one like the `example`'s
        // `limitless_ui`, and the `hosted` ngdart/ngcompiler, whose
        // `.template.dart` files DDC asks for). The files that the input's
        // program does not load are refused by the generator, with a reason.
        let Some((program, program_names)) = ctx.program else {
            return Err("ngdart (stage A): no program loaded".into());
        };
        // On an existing resource, the package's list of conservative queries
        // gains no new terms. The engine keeps the previous answers and only
        // replaces this file's digest.
        if let Some(output) = self.try_resource(ctx, request) {
            return Ok(output);
        }
        if let Some(output) = self.try_dart(ctx, request) {
            return Ok(output);
        }
        let t = std::time::Instant::now();
        let mut v = Vec::new();
        for d in ["lib", "web", "test"] {
            let dir = root.join(d);
            ctx.register(Query::Glob {
                dir: dartforge_elements::gerado::chave(&dir),
                pattern: "**".into(),
            });
            collect_files(&dir, &mut v);
        }
        // A `.dart` without any Angular marker only enters the generation
        // through its API (the `Resolvedor` asks where a type is declared and
        // the type of a public member; its template is the trivial one, which
        // only depends on the name). For it, the query is the library's API:
        // editing a body there does not rerun ngdart. The rest — component,
        // directive, pipe, injector, `.html`, `.scss`, `.css` — is the whole
        // text.
        let library_of: std::collections::HashMap<std::path::PathBuf, &str> = program
            .units
            .iter()
            .filter_map(|u| {
                let p = u.path.as_ref()?;
                Some((
                    dartforge_elements::gerado::chave(p),
                    program.library(u.library).uri.as_str(),
                ))
            })
            .collect();
        for p in &v {
            let k = dartforge_elements::gerado::chave(p);
            let dart = p.extension().is_some_and(|x| x == "dart");
            let uri = library_of.get(&k).copied();
            match (dart, uri) {
                (true, Some(uri))
                    if !looks_angular(&std::fs::read_to_string(p).unwrap_or_default()) =>
                {
                    ctx.register(Query::LibraryApi(uri.to_string()));
                    // The file name decides the trivial template: its
                    // existence is also a query.
                    ctx.register(Query::Exists(k));
                }
                _ => ctx.register(Query::File(k)),
            }
        }
        // What the `Resolvedor` may ask from outside the package: the
        // libraries the package's libraries reach through `import`/`export`
        // (generating a file only sees its own scope). In a dependency,
        // editing the application does not wake up the dependency's round.
        let in_package =
            |l: &dartforge_elements::model::Library| match l.uri.strip_prefix("package:") {
                Some(r) => r.split('/').next() == Some(request.package.as_str()),
                None => l
                    .units
                    .first()
                    .and_then(|&u| program.unit(u).path.as_ref())
                    .is_some_and(|p| p.starts_with(root)),
            };
        let mut seen = vec![false; program.libraries.len()];
        let mut queue: Vec<usize> = (0..program.libraries.len())
            .filter(|&i| !program.libraries[i].is_sdk && in_package(&program.libraries[i]))
            .collect();
        while let Some(i) = queue.pop() {
            if std::mem::replace(&mut seen[i], true) {
                continue;
            }
            let l = &program.libraries[i];
            queue.extend(
                l.imports
                    .iter()
                    .map(|x| x.library.0 as usize)
                    .chain(l.exports.iter().map(|x| x.library.0 as usize)),
            );
            if !l.is_sdk && !in_package(l) {
                ctx.register(Query::LibrarySource(l.uri.clone()));
            }
        }
        let t_queries = t.elapsed();
        let package = dartforge_ng_generator::Pacote {
            nome: request.package.clone(),
            raiz: root.clone(),
            folhas_geradas: generated_sheets(ctx, root),
        };
        let resolver =
            dartforge_ng_generator::resolucao::Resolvedor::novo(program, program_names);
        let mut names = dartforge_intern::Interner::new();
        let (g, scoreboard) =
            dartforge_ng_generator::gerar_com_apoio(&package, &mut names, None, Some(&resolver));
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!(
                "ngdart (stage A): queries {:.1} ms ({} files), gerar_com_apoio {:.1} ms",
                t_queries.as_secs_f64() * 1000.0,
                v.len(),
                (t.elapsed() - t_queries).as_secs_f64() * 1000.0
            );
        }
        let mut s = NativeOutput::default();
        let mut resource_readers: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        // The generator writes for every file in the directories; the engine
        // keeps only the outputs the plan expects (`build.yaml`'s
        // `generate_for` removes, for example, the stylesheets of
        // `web/assets/`).
        let planned = planned_outputs(request);
        for (p, f) in g.iter() {
            if f.gerador == "ngdart" && planned.contains(&dartforge_elements::gerado::chave(p)) {
                s.outputs.insert(p.clone(), f.conteudo.as_bytes().to_vec());
                if p.to_string_lossy().ends_with(".template.dart") {
                    if let Some(source) = f.entradas.first() {
                        for resource in f.entradas.iter().filter(|e| {
                            e.extension().is_some_and(|x| {
                                matches!(x.to_str(), Some("html" | "css" | "scss" | "sass"))
                            })
                        }) {
                            resource_readers
                                .entry(dartforge_elements::gerado::chave(resource))
                                .or_default()
                                .push(source.clone());
                        }
                    }
                }
            }
        }
        for (i, p) in scoreboard.pendentes.iter().enumerate() {
            // A refused stylesheet comes through its output: the refusal
            // belongs to the input.
            let p = &sheet_of_output(p).unwrap_or_else(|| p.clone());
            let m = scoreboard
                .conjuntos
                .get(i)
                .and_then(|c| c.iter().next())
                .map(|m| format!("ngdart (stage A) refuses: {m:?}"))
                .unwrap_or_else(|| "ngdart (stage A) refuses".into());
            s.refusals.insert(p.clone(), m);
        }
        s.generated_unit_count = scoreboard.gerados;
        self.cache
            .lock()
            .map_err(|_| "ngdart: poisoned cache")?
            .insert(
                request.package.clone(),
                NgCache {
                    output: clone_output(&s),
                    resource_readers,
                    index: None,
                },
            );
        Ok(s)
    }
}

/// The outputs the plan expects from the request's actions, by key.
fn planned_outputs(request: &NativeRequest) -> std::collections::HashSet<PathBuf> {
    request
        .actions
        .iter()
        .flat_map(|a| {
            a.outputs
                .iter()
                .map(|(_, p)| dartforge_elements::gerado::chave(p))
        })
        .collect()
}

/// The `x.css` stylesheet of an `x.css.dart`/`x.css.shim.dart` output.
fn sheet_of_output(p: &Path) -> Option<PathBuf> {
    let name = p.file_name()?.to_string_lossy();
    let base = name
        .strip_suffix(".shim.dart")
        .or_else(|| name.strip_suffix(".dart"))?;
    base.ends_with(".css").then(|| p.with_file_name(base))
}

fn clone_output(s: &NativeOutput) -> NativeOutput {
    NativeOutput {
        outputs: s.outputs.clone(),
        refusals: s.refusals.clone(),
        generated_unit_count: s.generated_unit_count,
        reuse_queries: s.reuse_queries,
    }
}

impl NgStageA {
    /// Edit to a `.dart` of the package (B03): regenerates only the files that
    /// may depend on what changed — the file itself and whoever reaches it
    /// through `import`/`export`, transitively, in the new program. Everything
    /// the generation of a file reads from another library (a `directives:`
    /// child, a resolved type, metadata inherited from a superclass, a
    /// selector) can only come from a library reachable through its imports:
    /// this closure covers, with room to spare, the queries that
    /// `gerar_arquivo` records. The closure's files are reindexed before
    /// generating.
    ///
    /// Falls back to stage A (returns `None`) for what it cannot handle this
    /// way: a file outside the package, a part, a file without a previous
    /// template, a file refused before or now, an output that did not exist.
    fn try_dart(&self, ctx: &mut GeneratorCtx<'_>, request: &NativeRequest) -> Option<NativeOutput> {
        let root = &request.package_root;
        let canon_root = std::fs::canonicalize(root).ok()?;
        // The events come in lexical and in canonical form: a single set.
        let mut changed: Vec<PathBuf> = Vec::new();
        for p in ctx.changed_paths.iter() {
            let c = std::fs::canonicalize(p).ok()?;
            if !changed.contains(&c) {
                changed.push(c);
            }
        }
        if changed.is_empty()
            || !changed.iter().all(|p| {
                p.starts_with(&canon_root)
                    && p.extension().is_some_and(|e| e == "dart")
                    && !p.to_string_lossy().ends_with(".template.dart")
            })
        {
            return None;
        }
        let (program, program_names) = ctx.program?;
        // Unit (canonical) → library, and the reverse import/export graph of
        // all the loaded libraries.
        let mut library_of_unit: HashMap<PathBuf, usize> = HashMap::new();
        for (i, l) in program.libraries.iter().enumerate() {
            for &u in &l.units {
                if let Some(p) = program
                    .unit(u)
                    .path
                    .as_ref()
                    .and_then(|p| std::fs::canonicalize(p).ok())
                {
                    library_of_unit.insert(p, i);
                }
            }
        }
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); program.libraries.len()];
        for (i, l) in program.libraries.iter().enumerate() {
            let targets = l
                .imports
                .iter()
                .map(|x| x.library)
                .chain(l.exports.iter().map(|x| x.library));
            for target in targets {
                if let Some(d) = dependents.get_mut(target.0 as usize) {
                    d.push(i);
                }
            }
        }
        let mut queue: Vec<usize> = Vec::new();
        for p in &changed {
            let &l = library_of_unit.get(p)?;
            // Only the defining unit: a part changes the whole library, and
            // stage A takes care of that.
            let main = program.libraries[l]
                .units
                .first()
                .and_then(|&u| program.unit(u).path.as_ref());
            if main
                .and_then(|x| std::fs::canonicalize(x).ok())
                .as_ref()
                != Some(p)
            {
                return None;
            }
            queue.push(l);
        }
        let mut seen = vec![false; program.libraries.len()];
        let mut closure: Vec<PathBuf> = Vec::new();
        while let Some(l) = queue.pop() {
            if std::mem::replace(&mut seen[l], true) {
                continue;
            }
            queue.extend(dependents[l].iter().copied());
            let lib = &program.libraries[l];
            if lib.is_sdk {
                continue;
            }
            let Some(p) = lib
                .units
                .first()
                .and_then(|&u| program.unit(u).path.as_ref())
            else {
                continue;
            };
            let Ok(c) = std::fs::canonicalize(p) else {
                continue;
            };
            let generated = c.to_string_lossy().ends_with(".template.dart");
            if c.starts_with(&canon_root) && !generated {
                closure.push(dartforge_elements::gerado::chave(p));
            }
        }
        closure.sort();
        let mut cache = self.cache.lock().ok()?;
        let cache = cache.get_mut(&request.package)?;
        let package = dartforge_ng_generator::Pacote {
            nome: request.package.clone(),
            raiz: root.clone(),
            folhas_geradas: generated_sheets(ctx, root),
        };
        let resolver =
            dartforge_ng_generator::resolucao::Resolvedor::novo(program, program_names);
        for source in &closure {
            let target = dartforge_ng_generator::caminho_do_template(source);
            if !cache.output.outputs.contains_key(&target)
                || cache.output.refusals.contains_key(source)
            {
                return None;
            }
        }
        // The index: from the new program, if it does not exist yet;
        // otherwise, the closure's files are reindexed (the others do not
        // reach what changed).
        let mut findings = Vec::with_capacity(closure.len());
        let mut names = dartforge_intern::Interner::new();
        for source in &closure {
            let text = std::fs::read_to_string(source).ok()?;
            let found = dartforge_ng_generator::analisar_arquivo(source, &text, &mut names);
            findings.push(found);
        }
        match cache.index.as_mut() {
            None => cache.index = Some(package_index(&package, &resolver)),
            Some(index) => {
                for (source, found) in closure.iter().zip(&findings) {
                    index.atualizar(&package, source, found, Some(&resolver));
                }
            }
        }
        let index = cache.index.as_ref()?;
        let mut new = Vec::new();
        let mut resources: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
        for (source, found) in closure.iter().zip(&findings) {
            let output = dartforge_ng_generator::gerar_arquivo(
                &package,
                source,
                found,
                Some(&resolver),
                &mut names,
                index,
            )
            .ok()?;
            let target = dartforge_ng_generator::caminho_do_template(source);
            new.push((target, output.template.into_bytes()));
            for (target, text) in output.extras {
                if !cache.output.outputs.contains_key(&target) {
                    return None;
                }
                new.push((target, text.into_bytes()));
            }
            let read = output
                .entradas
                .iter()
                .filter(|e| {
                    e.extension().is_some_and(|x| {
                        matches!(x.to_str(), Some("html" | "css" | "scss" | "sass"))
                    })
                })
                .map(|e| dartforge_elements::gerado::chave(e))
                .collect();
            resources.push((source.clone(), read));
        }
        for (target, bytes) in new {
            cache.output.outputs.insert(target, bytes);
        }
        // The resources each file of the closure reads now.
        for (source, read) in resources {
            for readers in cache.resource_readers.values_mut() {
                readers.retain(|f| *f != source);
            }
            for r in read {
                cache
                    .resource_readers
                    .entry(r)
                    .or_default()
                    .push(source.clone());
            }
        }
        cache.resource_readers.retain(|_, v| !v.is_empty());
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!(
                "ngdart (stage B): {} file(s) in the closure of {} .dart edit(s)",
                closure.len(),
                changed.len()
            );
        }
        let mut output = clone_output(&cache.output);
        output.generated_unit_count = closure.len();
        output.reuse_queries = true;
        // Stage A's queries over the edited files, with the new digest: the
        // text (a file with Angular) and the API (without Angular).
        let library_of: HashMap<PathBuf, &str> = program
            .units
            .iter()
            .filter_map(|u| {
                Some((
                    dartforge_elements::gerado::chave(u.path.as_ref()?),
                    program.library(u.library).uri.as_str(),
                ))
            })
            .collect();
        for p in ctx.changed_paths.clone().iter() {
            let k = dartforge_elements::gerado::chave(p);
            ctx.register(Query::File(k.clone()));
            ctx.register(Query::Exists(k.clone()));
            if let Some(uri) = library_of.get(&k) {
                ctx.register(Query::LibraryApi(uri.to_string()));
            }
        }
        Some(output)
    }

    fn try_resource(
        &self,
        ctx: &mut GeneratorCtx<'_>,
        request: &NativeRequest,
    ) -> Option<NativeOutput> {
        let resource = ctx.changed_paths.iter().find(|p| {
            p.starts_with(&request.package_root)
                && p.extension()
                    .is_some_and(|e| matches!(e.to_str(), Some("html" | "css" | "scss" | "sass")))
        })?;
        // The engine includes the lexical and the canonical form of the same
        // event; on Windows they may have different roots (links/short
        // names). The shortcut is only allowed when *all* events are this
        // file.
        let canon_resource = std::fs::canonicalize(resource).ok()?;
        if ctx
            .changed_paths
            .iter()
            .any(|p| std::fs::canonicalize(p).ok().as_ref() != Some(&canon_resource))
        {
            return None;
        }
        let mut cache = self.cache.lock().ok()?;
        let cache = cache.get_mut(&request.package)?;
        // A stylesheet (the `.css`, or the `.scss` that `sass_builder`
        // compiles): only its outputs, which do not depend on any component.
        if resource
            .extension()
            .is_some_and(|e| matches!(e.to_str(), Some("css" | "scss" | "sass")))
        {
            let package = dartforge_ng_generator::Pacote {
                nome: request.package.clone(),
                raiz: request.package_root.clone(),
                folhas_geradas: generated_sheets(ctx, &request.package_root),
            };
            let css = resource.with_extension("css");
            let mut new = Vec::new();
            for (target, r) in dartforge_ng_generator::gerar_folha(&package, &css) {
                // An output that was not ours, or that stops being ours: the
                // full path decides.
                let bytes = r.ok()?.into_bytes();
                if !cache.output.outputs.contains_key(&target) {
                    return None;
                }
                new.push((target, bytes));
            }
            for (target, bytes) in new {
                cache.output.outputs.insert(target, bytes);
            }
            let mut output = clone_output(&cache.output);
            output.generated_unit_count = 1;
            output.reuse_queries = true;
            ctx.register(Query::File(resource.clone()));
            return Some(output);
        }
        let sources = cache.resource_readers.get(resource)?.clone();
        let (program, program_names) = ctx.program?;
        let package = dartforge_ng_generator::Pacote {
            nome: request.package.clone(),
            raiz: request.package_root.clone(),
            folhas_geradas: generated_sheets(ctx, &request.package_root),
        };
        let resolver =
            dartforge_ng_generator::resolucao::Resolvedor::novo(program, program_names);
        if cache.index.is_none() {
            cache.index = Some(package_index(&package, &resolver));
        }
        let index = cache.index.as_ref()?;
        let mut new = Vec::new();
        let components = sources.len();
        for source in sources {
            let text = std::fs::read_to_string(&source).ok()?;
            let mut names = dartforge_intern::Interner::new();
            let found = dartforge_ng_generator::analisar_arquivo(&source, &text, &mut names);
            let output = dartforge_ng_generator::gerar_arquivo(
                &package,
                &source,
                &found,
                Some(&resolver),
                &mut names,
                index,
            )
            .ok()?;
            let target = dartforge_ng_generator::caminho_do_template(&source);
            if !cache.output.outputs.contains_key(&target) {
                return None;
            }
            new.push((target, output.template.into_bytes()));
            for (target, text) in output.extras {
                if !cache.output.outputs.contains_key(&target) {
                    return None;
                }
                new.push((target, text.into_bytes()));
            }
        }
        for (target, bytes) in new {
            cache.output.outputs.insert(target, bytes);
        }
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!(
                "ngdart (stage B): {} component(s) for {}",
                cache.resource_readers[resource].len(),
                resource.display()
            );
        }
        let mut output = clone_output(&cache.output);
        output.generated_unit_count = components;
        output.reuse_queries = true;
        ctx.register(Query::File(resource.clone()));
        Some(output)
    }
}

fn package_index(
    package: &dartforge_ng_generator::Pacote,
    resolver: &dartforge_ng_generator::resolucao::Resolvedor<'_>,
) -> dartforge_ng_generator::Indice {
    let mut index = dartforge_ng_generator::Indice::do_programa(resolver);
    let mut dart_files = Vec::new();
    for dir in ["lib", "web", "test"] {
        collect_files(&package.raiz.join(dir), &mut dart_files);
    }
    let mut names = dartforge_intern::Interner::new();
    for source in dart_files {
        if source.extension().is_none_or(|e| e != "dart")
            || source.to_string_lossy().ends_with(".template.dart")
        {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&source) {
            let found =
                dartforge_ng_generator::analisar_arquivo(Path::new(&source), &text, &mut names);
            index.atualizar(package, &source, &found, Some(resolver));
        }
    }
    index
}
