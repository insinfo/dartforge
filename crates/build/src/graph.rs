//! The asset graph: each package's sources, synthetic nodes and the
//! **expected** outputs of each phase — computing is not running.
//!
//! Reproduces `asset_graph/graph.dart` (`_addOutputsForSources`,
//! `_addInBuildPhaseOutputs`, `_actionMatches`, `_addGeneratedOutputs`) and
//! the source listing of `build_definition.dart` (`_listAssetIds`) with the
//! visibility of `target_graph.dart`.
use crate::glob::Glob;
use crate::packages::{PackageGraph, SDK};
use crate::plan::{InputMatcher, Configs, Phase, TargetNode};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `AssetId`: package and POSIX path relative to the package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId {
    pub package: Arc<str>,
    pub path: Arc<str>,
}

impl AssetId {
    pub fn new(package: &str, path: &str) -> AssetId {
        AssetId {
            package: package.into(),
            path: path.into(),
        }
    }

    /// `package|path`, as `build` writes it.
    pub fn text(&self) -> String {
        format!("{}|{}", self.package, self.path)
    }

    pub fn from_text(s: &str) -> Option<AssetId> {
        let (p, c) = s.split_once('|')?;
        Some(AssetId::new(p, c))
    }
}

/// The visible patterns of a package that is not the root
/// (`options.dart`, `defaultNonRootVisibleAssets`).
pub const VISIBLE_OUTSIDE_ROOT: &[&str] = &[
    "CHANGELOG*",
    "lib/**",
    "bin/**",
    "LICENSE*",
    "pubspec.yaml",
    "README*",
];

/// Default sources of the root package (`options.dart`,
/// `defaultRootPackageSources`).
pub const ROOT_SOURCES: &[&str] = &[
    "assets/**",
    "benchmark/**",
    "bin/**",
    "CHANGELOG*",
    "example/**",
    "lib/**",
    "test/**",
    "integration_test/**",
    "tool/**",
    "web/**",
    "node/**",
    "LICENSE*",
    "pubspec.yaml",
    "pubspec.lock",
    "README*",
    "$package$",
];

/// Synthetic nodes of every package (`graph.dart`, `placeholderIdsFor`).
pub const SYNTHETIC: &[&str] = &["lib/$lib$", "test/$test$", "web/$web$", "$package$"];

/// An action: a phase applied to a primary input. No outputs = removed action
/// (the input is no longer a source), except for a post-processor anchor
/// (`pos`), whose outputs are only known when it runs
/// (`PostProcessAnchorNode`).
#[derive(Debug, Clone)]
pub struct Action {
    pub phase: usize,
    pub entrada: AssetId,
    pub outputs: Vec<AssetId>,
    pub pos: bool,
}

impl Action {
    /// The action exists in the current graph (it was not removed by
    /// `_removeRecursive`).
    pub fn live(&self) -> bool {
        self.pos || !self.outputs.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedNode {
    pub action: usize,
    pub phase: usize,
    pub hidden: bool,
}

#[derive(Debug, Default)]
pub struct Graph {
    /// Sources (including the synthetic ones), by package.
    pub sources: BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    pub generated: BTreeMap<AssetId, GeneratedNode>,
    pub actions: Vec<Action>,
    /// Packages whose sources were listed (those with some generating phase).
    pub listed: BTreeSet<Arc<str>>,
    /// Source filters of the unlisted packages: a builder read in one of them
    /// is decided path by path, without listing the package.
    pub externos: BTreeMap<Arc<str>, SourceFilter>,
}

/// Which paths of a package are sources: some target includes them (and does
/// not exclude them) and, outside the root, they are visible
/// (`target_graph.dart`).
#[derive(Debug, Clone)]
pub struct SourceFilter {
    pub targets: Vec<InputMatcher>,
    pub visible: Option<Vec<Glob>>,
}

impl SourceFilter {
    pub fn admits(&self, path: &str) -> bool {
        self.visible
            .as_ref()
            .is_none_or(|g| g.iter().any(|x| x.matches(path)))
            && self
                .targets
                .iter()
                .any(|c| c.include.as_ref().is_some_and(|i| !i.is_empty()) && c.matches(path))
    }
}

/// Lists, under `root`, the files that some of `globs` matches, pruning the
/// directories none of them can reach.
pub fn list(root: &Path, globs: &[Glob]) -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    let mut stack: Vec<(PathBuf, String)> = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(ls) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in ls.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let r = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let Ok(kind) = e.file_type() else { continue };
            let is_dir = kind.is_dir() || (kind.is_symlink() && e.path().is_dir());
            if is_dir {
                if globs.iter().any(|g| g.may_descend(&r)) {
                    stack.push((e.path(), r));
                }
            } else if globs.iter().any(|g| g.matches(&r)) {
                v.insert(r);
            }
        }
    }
    v
}

fn globs(patterns: &[String]) -> Result<Vec<Glob>, String> {
    patterns.iter().map(|p| Glob::new(p)).collect()
}

impl Graph {
    pub fn has_source(&self, id: &AssetId) -> bool {
        self.sources
            .get(&id.package)
            .is_some_and(|s| s.contains(&id.path))
    }

    pub fn existe(&self, id: &AssetId) -> bool {
        self.has_source(id) || self.generated.contains_key(id)
    }

    /// Original source of a chain of outputs.
    pub fn origin<'a>(&'a self, id: &'a AssetId) -> &'a AssetId {
        let mut current = id;
        while let Some(g) = self.generated.get(current) {
            current = &self.actions[g.action].entrada;
        }
        current
    }

    /// The source filter of a package (`_listAssetIds`): the `sources` of each
    /// target over the patterns, and the visibility outside the root. `None`
    /// for `$sdk` and packages without a directory.
    fn package_filter(
        graph: &PackageGraph,
        configs: &Configs,
        package: usize,
    ) -> Result<Option<SourceFilter>, String> {
        let node = &graph.nos[package];
        let cfg = &configs.by_package[package];
        if node.name == SDK || node.root.as_os_str().is_empty() {
            return Ok(None);
        }
        let base: &[&str] = if node.is_root {
            ROOT_SOURCES
        } else {
            VISIBLE_OUTSIDE_ROOT
        };
        let default: Vec<String> = base
            .iter()
            .map(|s| s.to_string())
            .chain(cfg.additional_public.iter().cloned())
            .collect();
        let visible = if node.is_root {
            None
        } else {
            Some(globs(&default)?)
        };
        let targets = cfg
            .targets
            .iter()
            .map(|target| InputMatcher::new(&target.sources, Some(&default)))
            .collect::<Result<_, _>>()?;
        Ok(Some(SourceFilter {
            targets: targets,
            visible: visible,
        }))
    }

    /// Sources of a package, by its targets (`_listAssetIds`).
    fn package_sources(
        graph: &PackageGraph,
        configs: &Configs,
        package: usize,
    ) -> Result<BTreeSet<String>, String> {
        let mut v = BTreeSet::new();
        let Some(filter) = Self::package_filter(graph, configs, package)? else {
            return Ok(v);
        };
        for matcher in &filter.targets {
            let inc = matcher.include.clone().unwrap_or_default();
            if inc.is_empty() {
                continue;
            }
            for c in list(&graph.nos[package].root, &inc) {
                if filter.admits(&c) {
                    v.insert(c);
                }
            }
        }
        Ok(v)
    }

    /// Is `id` a source of a package that the graph did not list (no phase
    /// generates in it), according to the package filter? It does not look
    /// at the disk. It is what `build_runner` answers to a builder that reads
    /// a dependency — `BuildStep.resolver` reads every imported library this
    /// way.
    pub fn external_source(&self, id: &AssetId) -> bool {
        !id.path.contains('$')
            && self
                .externos
                .get(&id.package)
                .is_some_and(|f| f.admits(&id.path))
    }

    /// Builds the graph: sources of the packages with generating phases,
    /// synthetic nodes for all of them, and the expected outputs phase by
    /// phase.
    pub fn build(
        graph: &PackageGraph,
        configs: &Configs,
        phases: &[Phase],
        _targets: &[TargetNode],
    ) -> Result<Graph, String> {
        let mut g = Graph::default();
        let packages_with_phase: BTreeSet<usize> = phases
            .iter()
            .filter(|f| f.extensions.is_some())
            .map(|f| f.package)
            .collect();
        for (i, node) in graph.nos.iter().enumerate() {
            let set = g.sources.entry(node.name.as_str().into()).or_default();
            for s in SYNTHETIC {
                set.insert((*s).into());
            }
            if packages_with_phase.contains(&i) {
                for c in Self::package_sources(graph, configs, i)? {
                    set.insert(c.into());
                }
                g.listed.insert(node.name.as_str().into());
            } else if let Some(f) = Self::package_filter(graph, configs, i)? {
                g.externos.insert(node.name.as_str().into(), f);
            }
        }
        // `allInputs`: sources + synthetic nodes, and the accumulated outputs.
        let mut inputs: BTreeMap<Arc<str>, BTreeSet<Arc<str>>> = g.sources.clone();
        for (fi, phase) in phases.iter().enumerate() {
            if phase.pos {
                g.anchors(fi, phase, graph, &inputs);
                continue;
            }
            let Some(ext) = &phase.extensions else { continue };
            let package: Arc<str> = graph.nos[phase.package].name.as_str().into();
            let candidates: Vec<Arc<str>> = inputs
                .get(&package)
                .map(|s| {
                    s.iter()
                        .filter(|c| phase.generate_for.matches(c) && ext.has_output(c))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            let mut new: Vec<Arc<str>> = Vec::new();
            for c in candidates {
                // It may have stopped being an input in this phase (it became
                // an output).
                if !inputs.get(&package).is_some_and(|s| s.contains(&c)) {
                    continue;
                }
                let id = AssetId {
                    package: package.clone(),
                    path: c.clone(),
                };
                if !phase.target_sources.matches(&g.origin(&id).path) {
                    continue;
                }
                let outputs: Vec<AssetId> = ext
                    .outputs(&c)?
                    .into_iter()
                    .map(|s| AssetId {
                        package: package.clone(),
                        path: s.into(),
                    })
                    .collect();
                let ai = g.actions.len();
                for s in &outputs {
                    if let Some(other) = g.generated.get(s) {
                        return Err(format!(
                            "Both {} and {} may output {}, which is not allowed.",
                            phases[other.phase].fabrica,
                            phase.fabrica,
                            s.text()
                        ));
                    }
                    // A source in the place of an output (a `source` output of
                    // a previous build): the output replaces it, and whatever
                    // had been expected from it goes away too
                    // (`_removeRecursive`).
                    if g.has_source(s) {
                        g.remove_recursive(s, &mut inputs, &mut new);
                    }
                    g.generated.insert(
                        s.clone(),
                        GeneratedNode {
                            action: ai,
                            phase: fi,
                            hidden: phase.hidden,
                        },
                    );
                    new.push(s.path.clone());
                }
                g.actions.push(Action {
                    phase: fi,
                    entrada: id,
                    outputs: outputs,
                    pos: false,
                });
            }
            let e = inputs.entry(package).or_default();
            e.extend(new);
        }
        Ok(g)
    }

    /// `_addPostBuildPhaseAnchors` with the `_actionMatches` of a
    /// `PostBuildAction`: one anchor per input of the package (sources and all
    /// the outputs of the build phases) that ends in one of the
    /// `inputExtensions`, matches `generate_for` and whose original source is
    /// in the target's `sources`. Without known `inputExtensions` (no Dart
    /// executor answered) the phase has no anchors.
    fn anchors(
        &mut self,
        fi: usize,
        phase: &Phase,
        graph: &PackageGraph,
        inputs: &BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    ) {
        let Some(ext) = &phase.post_inputs else {
            return;
        };
        let package: Arc<str> = graph.nos[phase.package].name.as_str().into();
        let Some(candidates) = inputs.get(&package) else {
            return;
        };
        for c in candidates {
            if !phase.generate_for.matches(c) || !ext.iter().any(|e| c.ends_with(e.as_str())) {
                continue;
            }
            let id = AssetId {
                package: package.clone(),
                path: c.clone(),
            };
            if !phase.target_sources.matches(&self.origin(&id).path) {
                continue;
            }
            self.actions.push(Action {
                phase: fi,
                entrada: id,
                outputs: Vec::new(),
                pos: true,
            });
        }
    }

    /// `_removeRecursive`: removes the node and, recursively, the outputs of
    /// the actions that had it as their primary input.
    fn remove_recursive(
        &mut self,
        id: &AssetId,
        inputs: &mut BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
        new: &mut Vec<Arc<str>>,
    ) {
        if let Some(f) = self.sources.get_mut(&id.package) {
            f.remove(&id.path);
        }
        if let Some(e) = inputs.get_mut(&id.package) {
            e.remove(&id.path);
        }
        // Outputs always belong to the input's package: `new` is from the
        // same phase.
        new.retain(|c| *c != id.path);
        self.generated.remove(id);
        let dependents: Vec<usize> = (0..self.actions.len())
            .filter(|&a| self.actions[a].entrada == *id && self.actions[a].live())
            .collect();
        for a in dependents {
            let outputs = std::mem::take(&mut self.actions[a].outputs);
            for s in outputs {
                self.remove_recursive(&s, inputs, new);
            }
        }
    }

    /// `build_impl.dart:443-463`: is the output readable by an action of the
    /// phase `reader`? A later phase, no; the same phase, only its own output.
    pub fn is_readable(&self, id: &AssetId, reader: usize, own: Option<usize>) -> bool {
        match self.generated.get(id) {
            None => self.has_source(id),
            Some(g) => g.phase < reader || (g.phase == reader && Some(g.action) == own),
        }
    }
}
