//! The package graph of `build_runner_core` 8.0.0
//! (`package_graph/package_graph.dart`, `PackageGraph.forPath`) and the SCCs of
//! `graphs` 2.3.2, which decide the order of packages and targets.
use crate::config::{Pubspec, DependencyType, LockedPackage, ler_lock};
use dartforge_elements::config::PackageConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct No {
    pub name: String,
    pub root: PathBuf,
    pub kind: DependencyType,
    pub is_root: bool,
    /// Direct dependencies, in the official order (sorted names).
    pub deps: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct PackageGraph {
    /// `allPackages`: those of `package_config.json` in name order, and
    /// `$sdk` at the end.
    pub nos: Vec<No>,
    pub root: usize,
    pub by_name: HashMap<String, usize>,
    /// Versions from `pubspec.lock`.
    pub lock: HashMap<String, LockedPackage>,
    /// Directory of `package_config.json` and `pubspec.lock`.
    pub root_dir: PathBuf,
}

/// Name of the SDK pseudo-package.
pub const SDK: &str = "$sdk";

impl PackageGraph {
    pub fn no(&self, name: &str) -> Option<&No> {
        self.by_name.get(name).map(|&i| &self.nos[i])
    }

    /// The project's version profile: the `build_config` and `build_runner`
    /// that `pubspec.lock` resolved ([`crate::profile`]).
    pub fn profile(&self) -> crate::profile::Profile {
        let v = |p: &str| self.lock.get(p).map(|t| t.version.as_str());
        crate::profile::Profile::from_versions(v("build_config"), v("build_runner"))
    }

    /// `PackageGraph.forPath(dir)`: `dir` has the root's `pubspec.yaml`;
    /// `package_config.json` is the one `cfg` already read.
    pub fn build(
        dir: &Path,
        cfg: &PackageConfig,
        sdk: Option<&Path>,
    ) -> Result<PackageGraph, String> {
        let pubspec = Pubspec::read(dir)?;
        let root_name = pubspec
            .name
            .clone()
            .ok_or("The current package has no name, please add one to the pubspec.yaml.")?;
        let root_dir = cfg
            .origin
            .as_ref()
            .and_then(|o| o.parent())
            .and_then(|d| d.parent())
            .map(Path::to_path_buf)
            .ok_or("package_config.json without an origin")?;
        let lock_path = root_dir.join("pubspec.lock");
        let lock_text = std::fs::read_to_string(&lock_path).map_err(|_| {
            "Unable to generate package graph, no `pubspec.lock` found. This program must be ran from the root directory of your package.".to_string()
        })?;
        let lock: HashMap<String, LockedPackage> =
            ler_lock(&lock_text, &lock_path.display().to_string())?
                .into_iter()
                .collect();

        let mut names: Vec<&String> = cfg.packages.keys().collect();
        names.sort();
        let mut nodes = Vec::with_capacity(names.len() + 1);
        let mut by_name = HashMap::new();
        for name in names {
            let info = &cfg.packages[name];
            let root = info
                .root_uri
                .to_file_path()
                .map(dartforge_elements::config::sem_verbatim)
                .map_err(|_| format!("invalid rootUri for {name}"))?;
            by_name.insert(name.clone(), nodes.len());
            nodes.push(No {
                name: name.clone(),
                root: root,
                kind: lock
                    .get(name)
                    .map(|t| t.kind)
                    .unwrap_or(DependencyType::Path),
                is_root: *name == root_name,
                deps: Vec::new(),
            });
        }
        let root = *by_name.get(&root_name).ok_or_else(|| {
            format!("Dependency {root_name} not present, please run `dart pub get` or `flutter pub get` to fetch dependencies.")
        })?;
        let find = |n: &str, parent: &str, by_name: &HashMap<String, usize>| {
            by_name.get(n).copied().ok_or_else(|| {
                format!("Dependency {n} of {parent} not present, please run `dart pub get` or `flutter pub get` to fetch dependencies.")
            })
        };
        // Root: dependencies ∪ dev_dependencies, sorted.
        let mut d: Vec<String> = pubspec.dependencies.clone();
        for x in &pubspec.dev_dependencies {
            if !d.contains(x) {
                d.push(x.clone());
            }
        }
        d.sort();
        nodes[root].deps = d
            .iter()
            .map(|n| find(n, &root_name, &by_name))
            .collect::<Result<_, _>>()?;
        for i in 0..nodes.len() {
            if i == root {
                continue;
            }
            let p = Pubspec::read(&nodes[i].root)?;
            let mut d = p.dependencies.clone();
            d.sort();
            d.dedup();
            let name = nodes[i].name.clone();
            nodes[i].deps = d
                .iter()
                .map(|n| find(n, &name, &by_name))
                .collect::<Result<_, _>>()?;
        }
        by_name.insert(SDK.to_string(), nodes.len());
        nodes.push(No {
            name: SDK.to_string(),
            root: sdk.map(Path::to_path_buf).unwrap_or_default(),
            kind: DependencyType::Hosted,
            is_root: false,
            deps: Vec::new(),
        });
        Ok(PackageGraph {
            nos: nodes,
            root: root,
            by_name: by_name,
            lock,
            root_dir: root_dir,
        })
    }

    /// Package order for the build script: SCCs from the root, dependencies
    /// first (`build_script_generate.dart:86-92`).
    pub fn script_order(&self) -> Vec<usize> {
        scc(&[self.root], |n| self.nos[n].deps.clone())
            .into_iter()
            .flatten()
            .collect()
    }
}

/// `stronglyConnectedComponents` of `graphs` 2.3.2, in the same visiting
/// order (explicit stack), so that components and ties come out the same.
pub fn scc(start: &[usize], edges: impl Fn(usize) -> Vec<usize>) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut lowlink: HashMap<usize, usize> = HashMap::new();
    let mut index: HashMap<usize, usize> = HashMap::new();
    let mut on_stack = std::collections::HashSet::new();
    let mut next = 0usize;
    let mut visited: Vec<usize> = Vec::new();
    // (node, iterator: edges and current position), `None` = not started yet.
    // Dart builds `[for (node in nodes) _StackState(node)]` and pops from the
    // end: the last node is visited first. This reproduces exactly that.
    let mut stack: Vec<(usize, Option<(Vec<usize>, usize)>)> =
        start.iter().map(|&n| (n, None)).collect();
    'outer: while let Some((node, it)) = stack.pop() {
        let (mut it, mut low) = match it {
            None => {
                if index.contains_key(&node) {
                    continue;
                }
                index.insert(node, next);
                lowlink.insert(node, next);
                let low = next;
                next += 1;
                let edges = edges(node);
                if edges.is_empty() {
                    result.push(vec![node]);
                    continue;
                }
                visited.push(node);
                on_stack.insert(node);
                ((edges, 0usize), low)
            }
            Some((es, pos)) => {
                let current = es[pos];
                let low = lowlink[&node].min(lowlink[&current]);
                ((es, pos), low)
            }
        };
        loop {
            let succ = it.0[it.1];
            if !index.contains_key(&succ) {
                stack.push((node, Some((it.0.clone(), it.1))));
                stack.push((succ, None));
                continue 'outer;
            } else if on_stack.contains(&succ) {
                low = low.min(index[&succ]);
                lowlink.insert(node, low);
            }
            it.1 += 1;
            if it.1 >= it.0.len() {
                break;
            }
        }
        if low == index[&node] {
            let mut comp = Vec::new();
            loop {
                let n = visited.pop().expect("empty Tarjan stack");
                on_stack.remove(&n);
                comp.push(n);
                if n == node {
                    break;
                }
            }
            result.push(comp);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scc_dependencies_first() {
        // 0 → 1 → 2, 0 → 2
        let g = [vec![1, 2], vec![2], vec![]];
        let r = scc(&[0], |n| g[n].clone());
        assert_eq!(r, vec![vec![2], vec![1], vec![0]]);
    }

    #[test]
    fn scc_cycle() {
        // 0 → 1 → 2 → 1
        let g = [vec![1], vec![2], vec![1]];
        let r = scc(&[0], |n| g[n].clone());
        assert_eq!(r, vec![vec![2, 1], vec![0]]);
    }

    #[test]
    fn scc_several_starts_in_dart_order() {
        // No edges: Dart pops from the end of the initial list.
        let g: [Vec<usize>; 3] = [vec![], vec![], vec![]];
        let r = scc(&[0, 1, 2], |n| g[n].clone());
        assert_eq!(r, vec![vec![2], vec![1], vec![0]]);
    }
}
