//! The plan: the list of builder applications that `build_runner` writes to
//! `.dart_tool/build/entrypoint/build.dart` (`build_script_generate.dart`) and
//! the phases it expands at run time (`apply_builders.dart`).
use crate::config::{AutoApply, BuildConfig, BuildTo, InputSet};
use crate::extensions::Extensions;
use crate::glob::Glob;
use crate::packages::{PackageGraph, SDK, scc};
use crate::value::Map;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyFilter {
    NoneByDefault,
    DependentsOf(String),
    All,
    Root,
}

impl ApplyFilter {
    fn text(&self) -> String {
        match self {
            ApplyFilter::NoneByDefault => "toNoneByDefault()".into(),
            ApplyFilter::DependentsOf(p) => {
                format!("toDependentsOf({})", serde_json::Value::String(p.clone()))
            }
            ApplyFilter::All => "toAllPackages()".into(),
            ApplyFilter::Root => "toRoot()".into(),
        }
    }
}

/// An application in `build.dart`: `apply(...)` or `applyPostProcess(...)`.
#[derive(Debug, Clone)]
pub struct Application {
    pub key: String,
    pub post_process: bool,
    /// Import as the script writes it (`package:…` or relative to
    /// `.dart_tool/build/entrypoint`).
    pub import: String,
    pub factories: Vec<String>,
    pub filter: ApplyFilter,
    pub optional: bool,
    pub hidden: bool,
    pub generate_for_pattern: Option<InputSet>,
    pub default_options: Map,
    pub dev_options: Map,
    pub release_options: Map,
    pub applies: Vec<String>,
    /// Package that defines the builder.
    pub package: String,
    /// Extensions from `build.yaml` (the run-time ones come from the
    /// descriptor).
    pub declared_extensions: Vec<(String, Vec<String>)>,
}

impl Application {
    /// Canonical form, comparable with the one read from `build.dart`.
    pub fn canonical_text(&self) -> String {
        let l = |v: &[String]| {
            crate::value::Value::List(
                v.iter()
                    .map(|s| crate::value::Value::Text(s.clone()))
                    .collect(),
            )
            .canonical_text()
        };
        let factories: Vec<String> = self
            .factories
            .iter()
            .map(|f| format!("{}#{f}", self.import))
            .collect();
        let gf = self
            .generate_for_pattern
            .as_ref()
            .map(|g| g.canonical_text())
            .unwrap_or_else(|| "-".into());
        if self.post_process {
            format!(
                "applyPostProcess {} {} defaultGenerateFor={gf} defaultOptions={} defaultDevOptions={} defaultReleaseOptions={}",
                serde_json::Value::String(self.key.clone()),
                l(&factories),
                self.default_options.canonical_text(),
                self.dev_options.canonical_text(),
                self.release_options.canonical_text(),
            )
        } else {
            format!(
                "apply {} {} {} isOptional={} hideOutput={} defaultGenerateFor={gf} defaultOptions={} defaultDevOptions={} defaultReleaseOptions={} appliesBuilders={}",
                serde_json::Value::String(self.key.clone()),
                l(&factories),
                self.filter.text(),
                self.optional,
                self.hidden,
                self.default_options.canonical_text(),
                self.dev_options.canonical_text(),
                self.release_options.canonical_text(),
                l(&self.applies),
            )
        }
    }

    fn applies_to_package(&self, graph: &PackageGraph, package: usize) -> bool {
        let node = &graph.nodes[package];
        match &self.filter {
            ApplyFilter::NoneByDefault => false,
            ApplyFilter::All => true,
            ApplyFilter::Root => node.is_root,
            ApplyFilter::DependentsOf(p) => node.deps.iter().any(|&d| graph.nodes[d].name == *p),
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
    pub by_package: Vec<BuildConfig>,
    pub warnings: Vec<String>,
    /// The `triggers` accumulated from all packages (`BuildConfigs.buildTriggers`).
    pub triggers: crate::triggers::Triggers,
}

/// `<package>.build.yaml` at the root and, with `--config <name>`,
/// `build.<name>.yaml` in place of the root's `build.yaml`
/// (`findBuildConfigOverrides`, the same in 2.4.15 and 2.16.1).
fn overrides(
    graph: &PackageGraph,
    config_key: Option<&str>,
) -> Result<HashMap<String, BuildConfig>, String> {
    let profile = graph.profile();
    let root = &graph.nodes[graph.root];
    let mut m = HashMap::new();
    if let Some(k) = config_key {
        let path = root.root.join(format!("build.{k}.yaml"));
        let text = std::fs::read_to_string(&path)
            .map_err(|_| format!("Cannot find build.{k}.yaml for specified config."))?;
        let deps: Vec<String> = root
            .deps
            .iter()
            .map(|&d| graph.nodes[d].name.clone())
            .collect();
        // The file's builder definitions are ignored (the official tool
        // warns): the plan comes from `build.yaml`; only targets, global
        // options and triggers count here.
        m.insert(
            root.name.clone(),
            BuildConfig::from_text_with_profile(
                &root.name,
                &deps,
                &text,
                &path.display().to_string(),
                profile,
            )?,
        );
    }
    let Ok(ls) = std::fs::read_dir(&root.root) else {
        return Ok(m);
    };
    let glob = Glob::new("*.build.yaml")?;
    let mut names: Vec<String> = ls
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        if !glob.matches(&name) {
            continue;
        }
        let package = name.split('.').next().unwrap_or_default().to_string();
        let Some(node) = graph.node(&package) else {
            continue;
        };
        let deps: Vec<String> = node
            .deps
            .iter()
            .map(|&d| graph.nodes[d].name.clone())
            .collect();
        // `--config` wins over `<root>.build.yaml` (it is inserted later).
        if config_key.is_some() && package == root.name {
            continue;
        }
        let path = root.root.join(&name);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        m.insert(
            package.clone(),
            BuildConfig::from_text_with_profile(
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
    pub fn read(graph: &PackageGraph, strict: bool) -> Result<Configs, String> {
        Self::read_with_config(graph, strict, None)
    }

    /// [`Configs::read`] with `--config <key>`: `build.<key>.yaml` in place of
    /// the root's `build.yaml` (only in the run-time configuration; the
    /// script — builder definitions and order — always reads `build.yaml`).
    ///
    /// # Errors
    /// As [`Configs::read`], plus a missing `build.<key>.yaml`.
    pub fn read_with_config(
        graph: &PackageGraph,
        strict: bool,
        config_key: Option<&str>,
    ) -> Result<Configs, String> {
        let profile = graph.profile();
        let ov = overrides(graph, config_key)?;
        let mut per_package = Vec::with_capacity(graph.nodes.len());
        let mut warnings = Vec::new();
        for node in &graph.nodes {
            let deps: Vec<String> = node
                .deps
                .iter()
                .map(|&d| graph.nodes[d].name.clone())
                .collect();
            if let Some(c) = ov.get(&node.name) {
                per_package.push(c.clone());
                continue;
            }
            if node.name == SDK {
                per_package.push(BuildConfig::pattern(SDK, &deps));
                continue;
            }
            match BuildConfig::from_dir_with_profile(&node.name, &deps, &node.root, profile) {
                Ok(c) => per_package.push(c),
                Err(e) if strict => {
                    return Err(format!(
                        "Failed to parse `build.yaml` for {}: {e}",
                        node.name
                    ));
                }
                Err(e) => {
                    warnings.push(e);
                    per_package.push(BuildConfig::pattern(&node.name, &deps));
                }
            }
        }
        let triggers = if profile.accepts_triggers() {
            crate::triggers::Triggers::from_configs(&per_package)
        } else {
            crate::triggers::Triggers::default()
        };
        if let Some(t) = triggers.warnings_text() {
            warnings.push(t);
        }
        Ok(Configs {
            by_package: per_package,
            warnings: warnings,
            triggers: triggers,
        })
    }
}

/// The contents of `build.dart`: the applications in their final order.
#[derive(Debug, Clone)]
pub struct Plan {
    pub applications: Vec<Application>,
    pub warnings: Vec<String>,
}

impl Plan {
    pub fn canonical_text(&self) -> String {
        self.applications
            .iter()
            .map(|a| a.canonical_text() + "\n")
            .collect()
    }

    pub fn application(&self, key: &str) -> Option<&Application> {
        self.applications.iter().find(|a| a.key == key)
    }

    /// `findBuildScriptOptions` (`build_script_generate.dart:72-160`).
    pub fn from_script(graph: &PackageGraph, configs: &Configs) -> Result<Plan, String> {
        let order = graph.script_order();
        let root = &graph.nodes[graph.root];
        let valid = |import: &str, package: &str| -> bool {
            if let Some(rest) = import.strip_prefix("package:") {
                let p = rest.split('/').next().unwrap_or_default();
                return graph.node(p).is_some();
            }
            package == root.name
        };
        let mut warnings = configs.warnings.clone();
        let mut defs = Vec::new();
        let mut post = Vec::new();
        for &i in &order {
            let c = &configs.by_package[i];
            for b in &c.builders {
                if valid(&b.import, &b.package) {
                    defs.push(b.clone());
                } else {
                    warnings.push(format!(
                        "Could not load imported package for definition \"{}\".",
                        b.key
                    ));
                }
            }
            for p in &c.post_process_builders {
                if valid(&p.import, &p.package) {
                    post.push(p.clone());
                }
            }
        }
        let root_cfg = &configs.by_package[*order.last().ok_or("empty package graph")?];
        // `findBuilderOrder` (`builder_ordering.dart`).
        defs.sort_by(|a, b| a.key.cmp(&b.key));
        if graph.profile().rejects_duplicate_key()
            && let Some(d) = defs.windows(2).find(|w| w[0].key == w[1].key)
        {
            return Err(format!("Duplicate builder key: {}", d[0].key));
        }
        let n = defs.len();
        let outputs: Vec<Vec<&String>> = defs
            .iter()
            .map(|d| d.extensions.iter().flat_map(|(_, s)| s).collect())
            .collect();
        let must_run_before = |child: usize, parent: usize| -> bool {
            defs[child].runs_before.contains(&defs[parent].key)
                || root_cfg
                    .global_for(&defs[child].key)
                    .is_some_and(|g| g.runs_before.contains(&defs[parent].key))
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
            .map(|i| (defs[i].key.clone(), i))
            .collect();
        let mut kahn = Vec::with_capacity(n);
        while let Some(first) = ready.iter().next().cloned() {
            ready.remove(&first);
            kahn.push(first.1);
            for &f in &edges[first.1] {
                in_degree[f] -= 1;
                if in_degree[f] == 0 {
                    ready.insert((defs[f].key.clone(), f));
                }
            }
        }
        if kahn.len() < n {
            let cycle: Vec<&str> = (0..n)
                .filter(|&i| in_degree[i] > 0)
                .map(|i| defs[i].key.as_str())
                .collect();
            return Err(format!("Required input cycle for [{}]", cycle.join(", ")));
        }
        kahn.reverse();
        for (key, _) in &root_cfg.global {
            let known = |k: &str| defs.iter().any(|d| d.key == k);
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
            applications.push(Application {
                key: d.key.clone(),
                post_process: false,
                import: script_import(&d.import),
                factories: d.factories.clone(),
                filter: match d.auto_apply {
                    AutoApply::None => ApplyFilter::NoneByDefault,
                    AutoApply::Dependents => ApplyFilter::DependentsOf(d.package.clone()),
                    AutoApply::AllPackages => ApplyFilter::All,
                    AutoApply::RootPackage => ApplyFilter::Root,
                },
                optional: d.optional,
                hidden: d.build_to == BuildTo::Cache,
                generate_for_pattern: d.defaults.generate_for.clone(),
                default_options: d.defaults.options.clone(),
                dev_options: d.defaults.dev_options.clone(),
                release_options: d.defaults.release_options.clone(),
                applies: d.applies_builders.clone(),
                package: d.package.clone(),
                declared_extensions: d.extensions.clone(),
            });
        }
        for p in post {
            applications.push(Application {
                key: p.key.clone(),
                post_process: true,
                import: script_import(&p.import),
                factories: vec![p.factory.clone()],
                filter: ApplyFilter::NoneByDefault,
                optional: false,
                hidden: p.build_to == BuildTo::Cache,
                generate_for_pattern: p.defaults.generate_for.clone(),
                default_options: p.defaults.options.clone(),
                dev_options: p.defaults.dev_options.clone(),
                release_options: p.defaults.release_options.clone(),
                applies: Vec::new(),
                package: p.package.clone(),
                declared_extensions: Vec::new(),
            });
        }
        Ok(Plan {
            applications: applications,
            warnings: warnings,
        })
    }
}

/// `InputMatcher`: empty or missing `include` = everything.
#[derive(Debug, Clone)]
pub struct InputMatcher {
    pub include: Option<Vec<Glob>>,
    pub exclude: Option<Vec<Glob>>,
}

impl InputMatcher {
    pub fn new(s: &InputSet, default_include: Option<&[String]>) -> Result<InputMatcher, String> {
        let inc = s.include.as_deref().or(default_include);
        Ok(InputMatcher {
            include: inc
                .map(|v| v.iter().map(|g| Glob::new(g)).collect::<Result<_, _>>())
                .transpose()?,
            exclude: s
                .exclude
                .as_ref()
                .map(|v| v.iter().map(|g| Glob::new(g)).collect::<Result<_, _>>())
                .transpose()?,
        })
    }

    pub fn includes(&self, path: &str) -> bool {
        match &self.include {
            None => true,
            Some(v) => v.is_empty() || v.iter().any(|g| g.matches(path)),
        }
    }

    pub fn excludes(&self, path: &str) -> bool {
        self.exclude
            .as_ref()
            .is_some_and(|v| !v.is_empty() && v.iter().any(|g| g.matches(path)))
    }

    pub fn matches(&self, path: &str) -> bool {
        self.includes(path) && !self.excludes(path)
    }

    pub fn text(&self) -> String {
        let l = |v: &Option<Vec<Glob>>| match v {
            None => "-".to_string(),
            Some(v) => v
                .iter()
                .map(|g| g.pattern.as_str())
                .collect::<Vec<_>>()
                .join(","),
        };
        format!("[{}]-[{}]", l(&self.include), l(&self.exclude))
    }
}

/// A phase (`InBuildPhase`, or an action of the final `PostBuildPhase`).
#[derive(Debug, Clone)]
pub struct Phase {
    pub application: usize,
    pub factory: String,
    pub package: usize,
    pub target: String,
    pub target_sources: InputMatcher,
    pub generate_for: InputMatcher,
    pub options: Map,
    pub root: bool,
    pub optional: bool,
    pub hidden: bool,
    pub post_process: bool,
    /// `None` for post-processors and replaced builders.
    pub extensions: Option<Extensions>,
    /// Post-processor: the `inputExtensions` of the `PostProcessBuilder`
    /// object (`build_runner` ignores the `input_extensions` in
    /// `build.yaml`). Only the Dart executor knows them; `None` while nobody
    /// has answered, and then the phase has no anchors
    /// (`_addPostBuildPhaseAnchors`).
    pub post_inputs: Option<Vec<String>>,
    pub replaced_by: Option<&'static str>,
    /// The factory is a known builder with fixed options
    /// ([`crate::equivalent`]): its key, and `options` are already the
    /// constructor's.
    pub equivalent: Option<&'static str>,
    /// `run_only_if_triggered: true` in the merged options and a profile with
    /// triggers ([`crate::triggers`]): the step only runs if triggered.
    pub only_if_triggered: bool,
}

impl Phase {
    /// `InBuildPhase.identity`: what changes the phase other than options.
    pub fn identity(&self, plan: &Plan, graph: &PackageGraph) -> String {
        // The labels (kept as they are) are hashed into the action
        // fingerprints and the keys of the saved state: changing them
        // invalidates existing caches.
        format!(
            "{}#{} {} {} gen={} fontes={} opcional={} oculta={} pos={} ext={:?} entradas_pos={:?} gatilho={}",
            plan.applications[self.application].key,
            self.factory,
            graph.nodes[self.package].name,
            self.target,
            self.generate_for.text(),
            self.target_sources.text(),
            self.optional,
            self.hidden,
            self.post_process,
            self.extensions.as_ref().map(|e| &e.declared),
            self.post_inputs,
            self.only_if_triggered,
        )
    }
}

/// A target of the target graph (`TargetNode`).
#[derive(Debug, Clone)]
pub struct TargetNode {
    pub package: usize,
    pub config: usize,
    pub target: usize,
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
pub fn phases(
    graph: &PackageGraph,
    configs: &Configs,
    plan: &Plan,
    release: bool,
    definitions: &crate::command_line::Defines,
) -> Result<(Vec<Phase>, Vec<TargetNode>), String> {
    let triggers_enabled = graph.profile().uses_triggers();
    // `allModules`: per package (in `allPackages` order), targets in order.
    let mut nodes: Vec<TargetNode> = Vec::new();
    let mut by_key: HashMap<&str, usize> = HashMap::new();
    for (p, c) in configs.by_package.iter().enumerate() {
        for (a, target) in c.targets.iter().enumerate() {
            by_key.insert(target.key.as_str(), nodes.len());
            nodes.push(TargetNode {
                package: p,
                config: p,
                target: a,
            });
        }
    }
    let target = |n: &TargetNode| &configs.by_package[n.config].targets[n.target];
    let mut target_deps: Vec<Vec<usize>> = Vec::with_capacity(nodes.len());
    for n in &nodes {
        let a = target(n);
        let mut v = Vec::new();
        for d in &a.dependencies {
            let Some(&i) = by_key.get(d.as_str()) else {
                return Err(format!(
                    "{} declares a dependency on {d} but it does not exist",
                    a.key
                ));
            };
            v.push(i);
        }
        target_deps.push(v);
    }
    let start: Vec<usize> = (0..nodes.len()).collect();
    let cycles = scc(&start, |i| target_deps[i].clone());

    let root_cfg = &configs.by_package[graph.root];
    let mut global: HashMap<&str, Map> = root_cfg
        .global
        .iter()
        .map(|(k, g)| {
            (
                k.as_str(),
                g.options.overlay(if release {
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
        global.insert(k.as_str(), base.overlay(m));
    }
    let by_application: HashMap<&str, usize> = plan
        .applications
        .iter()
        .enumerate()
        .map(|(i, a)| (a.key.as_str(), i))
        .collect();
    let mut applied_with: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, a) in plan.applications.iter().enumerate() {
        for b in &a.applies {
            applied_with.entry(b.as_str()).or_default().push(i);
        }
    }

    // `_shouldApply` with memory (the recursion through `applies_builders`
    // repeats).
    fn should_apply(
        a: usize,
        n: usize,
        graph: &PackageGraph,
        plan: &Plan,
        nodes: &[TargetNode],
        configs: &Configs,
        by_application: &HashMap<&str, usize>,
        applied_with: &HashMap<&str, Vec<usize>>,
        depth: usize,
    ) -> bool {
        let ap = &plan.applications[a];
        let node = &nodes[n];
        let package = &graph.nodes[node.package];
        let all_hidden = ap.applies.iter().all(|b| {
            by_application
                .get(b.as_str())
                .is_none_or(|&i| plan.applications[i].hidden)
        });
        if !(ap.hidden && all_hidden) && !package.is_root {
            return false;
        }
        let target = &configs.by_package[node.config].targets[node.target];
        if let Some(c) = target.cfg(&ap.key) {
            return c.enabled;
        }
        if target.auto_apply_builders && ap.applies_to_package(graph, node.package) {
            return true;
        }
        if depth > 64 {
            return false;
        }
        applied_with.get(ap.key.as_str()).is_some_and(|anchors| {
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

    let mut equivalents: HashMap<(usize, String), Option<crate::equivalent::Equivalent>> =
        HashMap::new();
    let mut in_build = Vec::new();
    let mut post = Vec::new();
    for cycle in &cycles {
        for (ia, ap) in plan.applications.iter().enumerate() {
            for fab in &ap.factories {
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
                    let cfg = a.cfg(&ap.key);
                    let empty = Map::default();
                    let target_options = cfg
                        .map(|c| {
                            c.options.overlay(if release {
                                &c.release_options
                            } else {
                                &c.dev_options
                            })
                        })
                        .unwrap_or_default()
                        .overlay(global.get(ap.key.as_str()).unwrap_or(&empty));
                    let options = ap
                        .default_options
                        .overlay(if release {
                            &ap.release_options
                        } else {
                            &ap.dev_options
                        })
                        .overlay(&target_options);
                    // `phase.options.config['run_only_if_triggered'] == true`,
                    // on the phase options before any equivalence.
                    let only_if_triggered = triggers_enabled
                        && !ap.post_process
                        && options.get("run_only_if_triggered")
                            == Some(&crate::value::Value::Bool(true));
                    if ap.post_process && !ap.hidden {
                        return Err(format!(
                            "{}: post_process_builders with build_to: source (build_config ≥ 1.3.0) is not supported by the DartForge engine yet",
                            ap.key
                        ));
                    }
                    let generate_for = cfg
                        .and_then(|c| c.generate_for.clone())
                        .or_else(|| ap.generate_for_pattern.clone())
                        .unwrap_or_default();
                    let replaced = crate::descriptor::replaced_by(&ap.key);
                    let equivalent = if ap.post_process || replaced.is_some() {
                        None
                    } else {
                        equivalents
                            .entry((ia, fab.clone()))
                            .or_insert_with(|| {
                                crate::equivalent::equivalent(
                                    graph,
                                    &ap.import,
                                    fab,
                                    &ap.declared_extensions,
                                )
                            })
                            .clone()
                    };
                    let options = match &equivalent {
                        Some(e) => e.options.clone(),
                        None => options,
                    };
                    let extensions = if ap.post_process || replaced.is_some() {
                        None
                    } else {
                        let decl = crate::descriptor::runtime_extensions(
                            &ap.key,
                            fab,
                            &ap.factories,
                            &options,
                            &ap.declared_extensions,
                        )?;
                        Some(Extensions::new_for(&decl, &ap.key)?)
                    };
                    let f = Phase {
                        application: ia,
                        factory: fab.clone(),
                        package: node.package,
                        target: a.key.clone(),
                        target_sources: InputMatcher::new(&a.sources, None)?,
                        generate_for: InputMatcher::new(&generate_for, None)?,
                        options: options,
                        root: graph.nodes[node.package].is_root,
                        optional: ap.optional,
                        hidden: ap.hidden,
                        post_process: ap.post_process,
                        extensions: extensions,
                        post_inputs: None,
                        replaced_by: replaced,
                        equivalent: equivalent.map(|e| e.key),
                        only_if_triggered,
                    };
                    if ap.post_process {
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
