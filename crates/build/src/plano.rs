//! The plan: the list of builder applications that `build_runner` writes to
//! `.dart_tool/build/entrypoint/build.dart` (`build_script_generate.dart`) and
//! the phases it expands at run time (`apply_builders.dart`).
use crate::config::{AutoApply, BuildConfig, BuildTo, InputSet};
use crate::extensoes::Extensoes;
use crate::glob::Glob;
use crate::pacotes::{GrafoPacotes, SDK, scc};
use crate::valor::Mapa;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filtro {
    Nenhum,
    DependentesDe(String),
    Todos,
    Raiz,
}

impl Filtro {
    fn text(&self) -> String {
        match self {
            Filtro::Nenhum => "toNoneByDefault()".into(),
            Filtro::DependentesDe(p) => {
                format!("toDependentsOf({})", serde_json::Value::String(p.clone()))
            }
            Filtro::Todos => "toAllPackages()".into(),
            Filtro::Raiz => "toRoot()".into(),
        }
    }
}

/// An application in `build.dart`: `apply(...)` or `applyPostProcess(...)`.
#[derive(Debug, Clone)]
pub struct Aplicacao {
    pub chave: String,
    pub pos: bool,
    /// Import as the script writes it (`package:…` or relative to
    /// `.dart_tool/build/entrypoint`).
    pub import: String,
    pub fabricas: Vec<String>,
    pub filtro: Filtro,
    pub opcional: bool,
    pub oculta: bool,
    pub generate_for_padrao: Option<InputSet>,
    pub opcoes_padrao: Mapa,
    pub opcoes_dev: Mapa,
    pub opcoes_release: Mapa,
    pub aplica: Vec<String>,
    /// Package that defines the builder.
    pub pacote: String,
    /// Extensions from `build.yaml` (the run-time ones come from the
    /// descriptor).
    pub extensoes_declaradas: Vec<(String, Vec<String>)>,
}

impl Aplicacao {
    /// Canonical form, comparable with the one read from `build.dart`.
    pub fn texto_canonico(&self) -> String {
        let l = |v: &[String]| {
            crate::valor::Valor::Lista(
                v.iter()
                    .map(|s| crate::valor::Valor::Texto(s.clone()))
                    .collect(),
            )
            .texto_canonico()
        };
        let factories: Vec<String> = self
            .fabricas
            .iter()
            .map(|f| format!("{}#{f}", self.import))
            .collect();
        let gf = self
            .generate_for_padrao
            .as_ref()
            .map(|g| g.texto_canonico())
            .unwrap_or_else(|| "-".into());
        if self.pos {
            format!(
                "applyPostProcess {} {} defaultGenerateFor={gf} defaultOptions={} defaultDevOptions={} defaultReleaseOptions={}",
                serde_json::Value::String(self.chave.clone()),
                l(&factories),
                self.opcoes_padrao.texto_canonico(),
                self.opcoes_dev.texto_canonico(),
                self.opcoes_release.texto_canonico(),
            )
        } else {
            format!(
                "apply {} {} {} isOptional={} hideOutput={} defaultGenerateFor={gf} defaultOptions={} defaultDevOptions={} defaultReleaseOptions={} appliesBuilders={}",
                serde_json::Value::String(self.chave.clone()),
                l(&factories),
                self.filtro.text(),
                self.opcional,
                self.oculta,
                self.opcoes_padrao.texto_canonico(),
                self.opcoes_dev.texto_canonico(),
                self.opcoes_release.texto_canonico(),
                l(&self.aplica),
            )
        }
    }

    fn applies_to_package(&self, graph: &GrafoPacotes, package: usize) -> bool {
        let node = &graph.nos[package];
        match &self.filtro {
            Filtro::Nenhum => false,
            Filtro::Todos => true,
            Filtro::Raiz => node.e_raiz,
            Filtro::DependentesDe(p) => node.deps.iter().any(|&d| graph.nos[d].nome == *p),
        }
    }
}

/// `_buildScriptImport` (`build_script_generate.dart:252-263`).
fn script_import(import: &str) -> String {
    if import.starts_with("package:") || import.starts_with("../") || import.starts_with('/') {
        return import.to_string();
    }
    // `p.url.relative(import, from: '.dart_tool/build/entrypoint')`.
    let mut parts: Vec<&str> = Vec::new();
    for p in import.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    let target = parts.join("/");
    if let Some(inside) = target.strip_prefix(".dart_tool/build/entrypoint/") {
        return inside.to_string();
    }
    if target.starts_with(".dart_tool/build/") {
        return format!("../{}", &target[".dart_tool/build/".len()..]);
    }
    if target.starts_with(".dart_tool/") {
        return format!("../../{}", &target[".dart_tool/".len()..]);
    }
    format!("../../../{target}")
}

/// The configuration of each package, as `build_runner` obtains it.
pub struct Configs {
    pub por_pacote: Vec<BuildConfig>,
    pub avisos: Vec<String>,
    /// The `triggers` accumulated from all packages (`BuildConfigs.buildTriggers`).
    pub gatilhos: crate::gatilhos::Gatilhos,
}

/// `<package>.build.yaml` at the root and, with `--config <name>`,
/// `build.<name>.yaml` in place of the root's `build.yaml`
/// (`findBuildConfigOverrides`, the same in 2.4.15 and 2.16.1).
fn overrides(
    graph: &GrafoPacotes,
    config_key: Option<&str>,
) -> Result<HashMap<String, BuildConfig>, String> {
    let profile = graph.perfil();
    let root = &graph.nos[graph.raiz];
    let mut m = HashMap::new();
    if let Some(k) = config_key {
        let path = root.raiz.join(format!("build.{k}.yaml"));
        let text = std::fs::read_to_string(&path)
            .map_err(|_| format!("Cannot find build.{k}.yaml for specified config."))?;
        let deps: Vec<String> = root.deps.iter().map(|&d| graph.nos[d].nome.clone()).collect();
        // The file's builder definitions are ignored (the official tool
        // warns): the plan comes from `build.yaml`; only targets, global
        // options and triggers count here.
        m.insert(
            root.nome.clone(),
            BuildConfig::de_texto_no_perfil(
                &root.nome,
                &deps,
                &text,
                &path.display().to_string(),
                profile,
            )?,
        );
    }
    let Ok(ls) = std::fs::read_dir(&root.raiz) else {
        return Ok(m);
    };
    let glob = Glob::novo("*.build.yaml")?;
    let mut names: Vec<String> = ls
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        if !glob.casa(&name) {
            continue;
        }
        let package = name.split('.').next().unwrap_or_default().to_string();
        let Some(node) = graph.no(&package) else {
            continue;
        };
        let deps: Vec<String> = node.deps.iter().map(|&d| graph.nos[d].nome.clone()).collect();
        // `--config` wins over `<root>.build.yaml` (it is inserted later).
        if config_key.is_some() && package == root.nome {
            continue;
        }
        let path = root.raiz.join(&name);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        m.insert(
            package.clone(),
            BuildConfig::de_texto_no_perfil(
                &package,
                &deps,
                &text,
                &path.display().to_string(),
                profile,
            )?,
        );
    }
    Ok(m)
}

impl Configs {
    /// `strict`: a `build.yaml` error is fatal (the run-time `TargetGraph`);
    /// otherwise it becomes the default with a warning (script generation).
    ///
    /// # Errors
    /// `build.yaml` rejected (with `strict`) or unreadable override.
    pub fn ler(graph: &GrafoPacotes, strict: bool) -> Result<Configs, String> {
        Self::read_with_config(graph, strict, None)
    }

    /// [`Configs::ler`] with `--config <key>`: `build.<key>.yaml` in place of
    /// the root's `build.yaml` (only in the run-time configuration; the
    /// script — builder definitions and order — always reads `build.yaml`).
    ///
    /// # Errors
    /// As [`Configs::ler`], plus a missing `build.<key>.yaml`.
    pub fn read_with_config(
        graph: &GrafoPacotes,
        strict: bool,
        config_key: Option<&str>,
    ) -> Result<Configs, String> {
        let profile = graph.perfil();
        let ov = overrides(graph, config_key)?;
        let mut per_package = Vec::with_capacity(graph.nos.len());
        let mut warnings = Vec::new();
        for node in &graph.nos {
            let deps: Vec<String> = node.deps.iter().map(|&d| graph.nos[d].nome.clone()).collect();
            if let Some(c) = ov.get(&node.nome) {
                per_package.push(c.clone());
                continue;
            }
            if node.nome == SDK {
                per_package.push(BuildConfig::padrao(SDK, &deps));
                continue;
            }
            match BuildConfig::from_dir_with_profile(&node.nome, &deps, &node.raiz, profile) {
                Ok(c) => per_package.push(c),
                Err(e) if strict => {
                    return Err(format!("Failed to parse `build.yaml` for {}: {e}", node.nome));
                }
                Err(e) => {
                    warnings.push(e);
                    per_package.push(BuildConfig::padrao(&node.nome, &deps));
                }
            }
        }
        let triggers = if profile.aceita_triggers() {
            crate::gatilhos::Gatilhos::das_configs(&per_package)
        } else {
            crate::gatilhos::Gatilhos::default()
        };
        if let Some(t) = triggers.texto_dos_avisos() {
            warnings.push(t);
        }
        Ok(Configs {
            por_pacote: per_package,
            avisos: warnings,
            gatilhos: triggers,
        })
    }
}

/// The contents of `build.dart`: the applications in their final order.
#[derive(Debug, Clone)]
pub struct Plano {
    pub aplicacoes: Vec<Aplicacao>,
    pub avisos: Vec<String>,
}

impl Plano {
    pub fn texto_canonico(&self) -> String {
        self.aplicacoes
            .iter()
            .map(|a| a.texto_canonico() + "\n")
            .collect()
    }

    pub fn aplicacao(&self, key: &str) -> Option<&Aplicacao> {
        self.aplicacoes.iter().find(|a| a.chave == key)
    }

    /// `findBuildScriptOptions` (`build_script_generate.dart:72-160`).
    pub fn do_script(graph: &GrafoPacotes, configs: &Configs) -> Result<Plano, String> {
        let order = graph.ordem_do_script();
        let root = &graph.nos[graph.raiz];
        let valid = |import: &str, package: &str| -> bool {
            if let Some(rest) = import.strip_prefix("package:") {
                let p = rest.split('/').next().unwrap_or_default();
                return graph.no(p).is_some();
            }
            package == root.nome
        };
        let mut warnings = configs.avisos.clone();
        let mut defs = Vec::new();
        let mut post = Vec::new();
        for &i in &order {
            let c = &configs.por_pacote[i];
            for b in &c.builders {
                if valid(&b.import, &b.pacote) {
                    defs.push(b.clone());
                } else {
                    warnings.push(format!(
                        "Could not load imported package for definition \"{}\".",
                        b.chave
                    ));
                }
            }
            for p in &c.pos {
                if valid(&p.import, &p.pacote) {
                    post.push(p.clone());
                }
            }
        }
        let root_cfg = &configs.por_pacote[*order.last().ok_or("empty package graph")?];
        // `findBuilderOrder` (`builder_ordering.dart`).
        defs.sort_by(|a, b| a.chave.cmp(&b.chave));
        if graph.perfil().recusa_chave_duplicada()
            && let Some(d) = defs.windows(2).find(|w| w[0].chave == w[1].chave)
        {
            return Err(format!("Duplicate builder key: {}", d[0].chave));
        }
        let n = defs.len();
        let outputs: Vec<Vec<&String>> = defs
            .iter()
            .map(|d| d.extensoes.iter().flat_map(|(_, s)| s).collect())
            .collect();
        let must_run_before = |child: usize, parent: usize| -> bool {
            defs[child].runs_before.contains(&defs[parent].chave)
                || root_cfg
                    .global_for(&defs[child].chave)
                    .is_some_and(|g| g.runs_before.contains(&defs[parent].chave))
        };
        let edges: Vec<Vec<usize>> =
            (0..n)
                .map(|parent| {
                    (0..n)
                        .filter(|&child| {
                            parent != child
                                && (defs[parent].required_inputs.iter().any(|ri| {
                                    outputs[child].iter().any(|s| s.ends_with(ri.as_str()))
                                }) || must_run_before(child, parent))
                        })
                        .collect()
                })
                .collect();
        // Kahn with a priority queue by key (`_topologicalSortWithSecondary`).
        let mut in_degree = vec![0usize; n];
        for a in &edges {
            for &f in a {
                in_degree[f] += 1;
            }
        }
        let mut ready: std::collections::BTreeSet<(String, usize)> = (0..n)
            .filter(|&i| in_degree[i] == 0)
            .map(|i| (defs[i].chave.clone(), i))
            .collect();
        let mut kahn = Vec::with_capacity(n);
        while let Some(first) = ready.iter().next().cloned() {
            ready.remove(&first);
            kahn.push(first.1);
            for &f in &edges[first.1] {
                in_degree[f] -= 1;
                if in_degree[f] == 0 {
                    ready.insert((defs[f].chave.clone(), f));
                }
            }
        }
        if kahn.len() < n {
            let cycle: Vec<&str> = (0..n)
                .filter(|&i| in_degree[i] > 0)
                .map(|i| defs[i].chave.as_str())
                .collect();
            return Err(format!("Required input cycle for [{}]", cycle.join(", ")));
        }
        kahn.reverse();
        for (key, _) in &root_cfg.global {
            let known = |k: &str| defs.iter().any(|d| d.chave == k);
            let g = root_cfg.global_for(key).expect("global key");
            for k in std::iter::once(key).chain(&g.runs_before) {
                if !known(k) {
                    warnings.push(format!(
                        "Invalid builder key `{k}` found in global_options config of build.yaml. This configuration will have no effect."
                    ));
                }
            }
        }
        let mut applications = Vec::with_capacity(n + post.len());
        for i in kahn {
            let d = &defs[i];
            applications.push(Aplicacao {
                chave: d.chave.clone(),
                pos: false,
                import: script_import(&d.import),
                fabricas: d.fabricas.clone(),
                filtro: match d.auto_apply {
                    AutoApply::None => Filtro::Nenhum,
                    AutoApply::Dependents => Filtro::DependentesDe(d.pacote.clone()),
                    AutoApply::AllPackages => Filtro::Todos,
                    AutoApply::RootPackage => Filtro::Raiz,
                },
                opcional: d.opcional,
                oculta: d.build_to == BuildTo::Cache,
                generate_for_padrao: d.defaults.generate_for.clone(),
                opcoes_padrao: d.defaults.options.clone(),
                opcoes_dev: d.defaults.dev_options.clone(),
                opcoes_release: d.defaults.release_options.clone(),
                aplica: d.applies_builders.clone(),
                pacote: d.pacote.clone(),
                extensoes_declaradas: d.extensoes.clone(),
            });
        }
        for p in post {
            applications.push(Aplicacao {
                chave: p.chave.clone(),
                pos: true,
                import: script_import(&p.import),
                fabricas: vec![p.fabrica.clone()],
                filtro: Filtro::Nenhum,
                opcional: false,
                oculta: p.build_to == BuildTo::Cache,
                generate_for_padrao: p.defaults.generate_for.clone(),
                opcoes_padrao: p.defaults.options.clone(),
                opcoes_dev: p.defaults.dev_options.clone(),
                opcoes_release: p.defaults.release_options.clone(),
                aplica: Vec::new(),
                pacote: p.pacote.clone(),
                extensoes_declaradas: Vec::new(),
            });
        }
        Ok(Plano {
            aplicacoes: applications,
            avisos: warnings,
        })
    }
}

/// `InputMatcher`: empty or missing `include` = everything.
#[derive(Debug, Clone)]
pub struct Casador {
    pub include: Option<Vec<Glob>>,
    pub exclude: Option<Vec<Glob>>,
}

impl Casador {
    pub fn novo(s: &InputSet, default_include: Option<&[String]>) -> Result<Casador, String> {
        let inc = s.include.as_deref().or(default_include);
        Ok(Casador {
            include: inc
                .map(|v| v.iter().map(|g| Glob::novo(g)).collect::<Result<_, _>>())
                .transpose()?,
            exclude: s
                .exclude
                .as_ref()
                .map(|v| v.iter().map(|g| Glob::novo(g)).collect::<Result<_, _>>())
                .transpose()?,
        })
    }

    pub fn inclui(&self, path: &str) -> bool {
        match &self.include {
            None => true,
            Some(v) => v.is_empty() || v.iter().any(|g| g.casa(path)),
        }
    }

    pub fn exclui(&self, path: &str) -> bool {
        self.exclude
            .as_ref()
            .is_some_and(|v| !v.is_empty() && v.iter().any(|g| g.casa(path)))
    }

    pub fn casa(&self, path: &str) -> bool {
        self.inclui(path) && !self.exclui(path)
    }

    pub fn texto(&self) -> String {
        let l = |v: &Option<Vec<Glob>>| match v {
            None => "-".to_string(),
            Some(v) => v
                .iter()
                .map(|g| g.padrao.as_str())
                .collect::<Vec<_>>()
                .join(","),
        };
        format!("[{}]-[{}]", l(&self.include), l(&self.exclude))
    }
}

/// A phase (`InBuildPhase`, or an action of the final `PostBuildPhase`).
#[derive(Debug, Clone)]
pub struct Fase {
    pub aplicacao: usize,
    pub fabrica: String,
    pub pacote: usize,
    pub alvo: String,
    pub fontes_alvo: Casador,
    pub generate_for: Casador,
    pub opcoes: Mapa,
    pub raiz: bool,
    pub opcional: bool,
    pub oculta: bool,
    pub pos: bool,
    /// `None` for post-processors and replaced builders.
    pub extensoes: Option<Extensoes>,
    /// Post-processor: the `inputExtensions` of the `PostProcessBuilder`
    /// object (`build_runner` ignores the `input_extensions` in
    /// `build.yaml`). Only the Dart executor knows them; `None` while nobody
    /// has answered, and then the phase has no anchors
    /// (`_addPostBuildPhaseAnchors`).
    pub entradas_pos: Option<Vec<String>>,
    pub substituido: Option<&'static str>,
    /// The factory is a known builder with fixed options
    /// ([`crate::equivalente`]): its key, and `opcoes` are already the
    /// constructor's.
    pub equivalente: Option<&'static str>,
    /// `run_only_if_triggered: true` in the merged options and a profile with
    /// triggers ([`crate::gatilhos`]): the step only runs if triggered.
    pub only_if_triggered: bool,
}

impl Fase {
    /// `InBuildPhase.identity`: what changes the phase other than options.
    pub fn identity(&self, plan: &Plano, graph: &GrafoPacotes) -> String {
        // The labels are hashed into the action fingerprints and the keys of
        // the saved state: changing them invalidates existing caches.
        format!(
            "{}#{} {} {} gen={} fontes={} opcional={} oculta={} pos={} ext={:?} entradas_pos={:?} gatilho={}",
            plan.aplicacoes[self.aplicacao].chave,
            self.fabrica,
            graph.nos[self.pacote].nome,
            self.alvo,
            self.generate_for.texto(),
            self.fontes_alvo.texto(),
            self.opcional,
            self.oculta,
            self.pos,
            self.extensoes.as_ref().map(|e| &e.declaradas),
            self.entradas_pos,
            self.only_if_triggered,
        )
    }
}

/// A target of the target graph (`TargetNode`).
#[derive(Debug, Clone)]
pub struct NoAlvo {
    pub pacote: usize,
    pub config: usize,
    pub alvo: usize,
}

/// The phases, like `createBuildPhases` (`apply_builders.dart:196-349` in
/// 2.4.15, `build_phase_creator.dart` in 2.16.1 — the same order and the same
/// option precedence). `definitions`: the `--define`s, merged by key over the
/// root's `global_options`.
///
/// # Errors
/// Dependency on a nonexistent target, invalid extensions, or a
/// post-processor with `build_to: source` (`build_config` ≥ 1.3.0), which the
/// engine does not run yet.
pub fn fases(
    graph: &GrafoPacotes,
    configs: &Configs,
    plan: &Plano,
    release: bool,
    definitions: &crate::linha_de_comando::Definicoes,
) -> Result<(Vec<Fase>, Vec<NoAlvo>), String> {
    let triggers_enabled = graph.perfil().dispara();
    // `allModules`: per package (in `allPackages` order), targets in order.
    let mut nodes: Vec<NoAlvo> = Vec::new();
    let mut by_key: HashMap<&str, usize> = HashMap::new();
    for (p, c) in configs.por_pacote.iter().enumerate() {
        for (a, target) in c.alvos.iter().enumerate() {
            by_key.insert(target.chave.as_str(), nodes.len());
            nodes.push(NoAlvo {
                pacote: p,
                config: p,
                alvo: a,
            });
        }
    }
    let target = |n: &NoAlvo| &configs.por_pacote[n.config].alvos[n.alvo];
    let mut target_deps: Vec<Vec<usize>> = Vec::with_capacity(nodes.len());
    for n in &nodes {
        let a = target(n);
        let mut v = Vec::new();
        for d in &a.dependencias {
            let Some(&i) = by_key.get(d.as_str()) else {
                return Err(format!(
                    "{} declares a dependency on {d} but it does not exist",
                    a.chave
                ));
            };
            v.push(i);
        }
        target_deps.push(v);
    }
    let start: Vec<usize> = (0..nodes.len()).collect();
    let cycles = scc(&start, |i| target_deps[i].clone());

    let root_cfg = &configs.por_pacote[graph.raiz];
    let mut global: HashMap<&str, Mapa> = root_cfg
        .global
        .iter()
        .map(|(k, g)| {
            (
                k.as_str(),
                g.options.sobrepor(if release {
                    &g.release_options
                } else {
                    &g.dev_options
                }),
            )
        })
        .collect();
    // `builderConfigOverrides`: `(global ?? empty).overrideWith(define)`.
    for (k, m) in &definitions.0 {
        let base = global.remove(k.as_str()).unwrap_or_default();
        global.insert(k.as_str(), base.sobrepor(m));
    }
    let by_application: HashMap<&str, usize> = plan
        .aplicacoes
        .iter()
        .enumerate()
        .map(|(i, a)| (a.chave.as_str(), i))
        .collect();
    let mut applied_with: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, a) in plan.aplicacoes.iter().enumerate() {
        for b in &a.aplica {
            applied_with.entry(b.as_str()).or_default().push(i);
        }
    }

    // `_shouldApply` with memory (the recursion through `applies_builders`
    // repeats).
    fn should_apply(
        a: usize,
        n: usize,
        graph: &GrafoPacotes,
        plan: &Plano,
        nodes: &[NoAlvo],
        configs: &Configs,
        by_application: &HashMap<&str, usize>,
        applied_with: &HashMap<&str, Vec<usize>>,
        depth: usize,
    ) -> bool {
        let ap = &plan.aplicacoes[a];
        let node = &nodes[n];
        let package = &graph.nos[node.pacote];
        let all_hidden = ap.aplica.iter().all(|b| {
            by_application
                .get(b.as_str())
                .is_none_or(|&i| plan.aplicacoes[i].oculta)
        });
        if !(ap.oculta && all_hidden) && !package.e_raiz {
            return false;
        }
        let target = &configs.por_pacote[node.config].alvos[node.alvo];
        if let Some(c) = target.cfg(&ap.chave) {
            return c.enabled;
        }
        if target.auto_apply_builders && ap.applies_to_package(graph, node.pacote) {
            return true;
        }
        if depth > 64 {
            return false;
        }
        applied_with.get(ap.chave.as_str()).is_some_and(|anchors| {
            anchors.iter().any(|&anchor| {
                should_apply(
                    anchor,
                    n,
                    graph,
                    plan,
                    nodes,
                    configs,
                    by_application,
                    applied_with,
                    depth + 1,
                )
            })
        })
    }

    let mut equivalents: HashMap<(usize, String), Option<crate::equivalente::Equivalente>> =
        HashMap::new();
    let mut in_build = Vec::new();
    let mut post = Vec::new();
    for cycle in &cycles {
        for (ia, ap) in plan.aplicacoes.iter().enumerate() {
            for fab in &ap.fabricas {
                for &n in cycle {
                    if !should_apply(
                        ia,
                        n,
                        graph,
                        plan,
                        &nodes,
                        configs,
                        &by_application,
                        &applied_with,
                        0,
                    ) {
                        continue;
                    }
                    let node = &nodes[n];
                    let a = target(node);
                    let cfg = a.cfg(&ap.chave);
                    let empty = Mapa::default();
                    let target_options = cfg
                        .map(|c| {
                            c.options.sobrepor(if release {
                                &c.release_options
                            } else {
                                &c.dev_options
                            })
                        })
                        .unwrap_or_default()
                        .sobrepor(global.get(ap.chave.as_str()).unwrap_or(&empty));
                    let options = ap
                        .opcoes_padrao
                        .sobrepor(if release {
                            &ap.opcoes_release
                        } else {
                            &ap.opcoes_dev
                        })
                        .sobrepor(&target_options);
                    // `phase.options.config['run_only_if_triggered'] == true`,
                    // on the phase options before any equivalence.
                    let only_if_triggered = triggers_enabled
                        && !ap.pos
                        && options.obter("run_only_if_triggered")
                            == Some(&crate::valor::Valor::Bool(true));
                    if ap.pos && !ap.oculta {
                        return Err(format!(
                            "{}: post_process_builders with build_to: source (build_config ≥ 1.3.0) is not supported by the DartForge engine yet",
                            ap.chave
                        ));
                    }
                    let generate_for = cfg
                        .and_then(|c| c.generate_for.clone())
                        .or_else(|| ap.generate_for_padrao.clone())
                        .unwrap_or_default();
                    let replaced = crate::descritor::substituido(&ap.chave);
                    let equivalent = if ap.pos || replaced.is_some() {
                        None
                    } else {
                        equivalents
                            .entry((ia, fab.clone()))
                            .or_insert_with(|| {
                                crate::equivalente::equivalente(
                                    graph,
                                    &ap.import,
                                    fab,
                                    &ap.extensoes_declaradas,
                                )
                            })
                            .clone()
                    };
                    let options = match &equivalent {
                        Some(e) => e.opcoes.clone(),
                        None => options,
                    };
                    let extensions = if ap.pos || replaced.is_some() {
                        None
                    } else {
                        let decl = crate::descritor::extensoes_de_execucao(
                            &ap.chave,
                            fab,
                            &ap.fabricas,
                            &options,
                            &ap.extensoes_declaradas,
                        )?;
                        Some(Extensoes::novas(&decl, &ap.chave)?)
                    };
                    let f = Fase {
                        aplicacao: ia,
                        fabrica: fab.clone(),
                        pacote: node.pacote,
                        alvo: a.chave.clone(),
                        fontes_alvo: Casador::novo(&a.sources, None)?,
                        generate_for: Casador::novo(&generate_for, None)?,
                        opcoes: options,
                        raiz: graph.nos[node.pacote].e_raiz,
                        opcional: ap.opcional,
                        oculta: ap.oculta,
                        pos: ap.pos,
                        extensoes: extensions,
                        entradas_pos: None,
                        substituido: replaced,
                        equivalente: equivalent.map(|e| e.chave),
                        only_if_triggered,
                    };
                    if ap.pos {
                        post.push(f);
                    } else {
                        in_build.push(f);
                    }
                }
            }
        }
    }
    in_build.extend(post);
    Ok((in_build, nodes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_script_import() {
        assert_eq!(script_import("package:a/b.dart"), "package:a/b.dart");
        assert_eq!(
            script_import("tool/builders.dart"),
            "../../../tool/builders.dart"
        );
        assert_eq!(
            script_import("./tool/builders.dart"),
            "../../../tool/builders.dart"
        );
    }
}
