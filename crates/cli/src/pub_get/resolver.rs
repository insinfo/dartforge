//! Deterministic dependency search with lockfile preferences and backtracking.
use super::{
    constraints::Constraint,
    hosted::{Candidate, Hosted},
    manifest::{Dependency, Manifest, Source},
};
use semver::Version;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Selected {
    pub source: Source,
    pub candidate: Candidate,
    /// The path that selected this package was written relative to its
    /// dependent; only meaningful for path sources.
    pub relative: bool,
}
enum SearchError {
    Conflict(String),
    Fatal(String),
}
impl From<String> for SearchError {
    fn from(error: String) -> Self {
        Self::Fatal(error)
    }
}
#[derive(Clone)]
struct Required {
    source: Source,
    relative: bool,
    constraints: Vec<(String, Constraint)>,
}
pub(super) struct Resolver {
    pub hosted: Hosted,
    pub git: super::git::Git,
    /// Lockfile content identities: hosted archive hashes and Git commits.
    pub locked_digests: BTreeMap<String, String>,
    pub sdk: Version,
    pub locked: BTreeMap<String, (Source, Version)>,
    pub sdk_root: Option<std::path::PathBuf>,
    attempts: usize,
    overrides: BTreeMap<String, Dependency>,
    root_name: String,
    /// Names of the workspace packages: every dependency on one of them is
    /// satisfied by the local package, whatever source it names.
    workspace: BTreeSet<String>,
}
impl Resolver {
    pub fn new(hosted: Hosted, sdk: Version, locked: BTreeMap<String, (Source, Version)>) -> Self {
        Self {
            git: super::git::Git::new(hosted.cache.clone(), hosted.offline),
            hosted,
            locked_digests: BTreeMap::new(),
            sdk,
            locked,
            sdk_root: None,
            attempts: 0,
            overrides: BTreeMap::new(),
            root_name: String::new(),
            workspace: BTreeSet::new(),
        }
    }
    /// Resolves a package that is not part of a larger workspace.
    #[cfg(test)]
    pub fn resolve(
        &mut self,
        root: &Manifest,
        directory: &std::path::Path,
    ) -> Result<BTreeMap<String, Selected>, String> {
        self.resolve_workspace(&[(root.clone(), directory.to_path_buf())])
    }

    /// Resolves a workspace together: the first package is the root, and the
    /// dependencies, development dependencies and overrides of every
    /// workspace package take part, as in Pub 3.6.
    pub fn resolve_workspace(
        &mut self,
        workspace: &[(Manifest, std::path::PathBuf)],
    ) -> Result<BTreeMap<String, Selected>, String> {
        let root = &workspace.first().ok_or("empty workspace")?.0;
        self.attempts = 0;
        self.root_name = root.name.clone();
        self.workspace = workspace.iter().map(|(m, _)| m.name.clone()).collect();
        if self.workspace.len() != workspace.len() {
            return Err("workspace package names must be unique".into());
        }
        self.overrides = BTreeMap::new();
        for (manifest, _) in workspace {
            check_sdk(manifest, &self.sdk)?;
            for (name, dependency) in &manifest.overrides {
                if let Some(existing) = self.overrides.get(name)
                    && (existing.source != dependency.source
                        || existing.constraint.text != dependency.constraint.text)
                {
                    return Err(format!(
                        "conflicting dependency_overrides for {name} in the workspace"
                    ));
                }
                self.overrides.insert(name.clone(), dependency.clone());
            }
        }
        let mut requirements = BTreeMap::new();
        for (manifest, _) in workspace {
            for dependencies in [&manifest.dependencies, &manifest.dev, &manifest.overrides] {
                add(
                    &mut requirements,
                    dependencies,
                    &manifest.name,
                    &self.overrides,
                    &self.workspace,
                )?;
            }
        }
        let mut selected = BTreeMap::new();
        for (manifest, directory) in workspace {
            selected.insert(
                manifest.name.clone(),
                Selected {
                    source: Source::Path(directory.clone()),
                    candidate: Candidate {
                        manifest: manifest.clone(),
                        manifest_error: None,
                        directory: directory.clone(),
                        archive: None,
                        digest: None,
                    },
                    relative: true,
                },
            );
        }
        let mut result = self.search(requirements, selected).map_err(|e| match e {
            SearchError::Conflict(s) | SearchError::Fatal(s) => s,
        })?;
        for (manifest, _) in workspace {
            result.remove(&manifest.name);
        }
        Ok(result)
    }
    fn search(
        &mut self,
        requirements: BTreeMap<String, Required>,
        selected: BTreeMap<String, Selected>,
    ) -> Result<BTreeMap<String, Selected>, SearchError> {
        if requirements.len() > 512 {
            return Err(SearchError::Fatal(
                "dependency graph exceeds 512-package limit".into(),
            ));
        }
        self.attempts += 1;
        if self.attempts > 100_000 {
            return Err(SearchError::Fatal(
                "dependency search exceeded 100000 decisions".into(),
            ));
        }
        for (name, requirement) in &requirements {
            if let Some(value) = selected.get(name)
                && ((value.source != requirement.source && !self.workspace.contains(name))
                    || !requirement
                        .constraints
                        .iter()
                        .all(|(_, c)| c.allows(&value.candidate.manifest.version)))
            {
                return Err(SearchError::Conflict(conflict(name, requirement)));
            }
        }
        let Some((name, requirement)) = requirements
            .iter()
            .find(|(name, _)| !selected.contains_key(*name))
        else {
            return Ok(selected);
        };
        let mut candidates = match &requirement.source {
            Source::Path(path) => vec![Candidate {
                manifest: Manifest::read(path, &self.hosted.default_host, false)?,
                manifest_error: None,
                directory: path.clone(),
                archive: None,
                digest: None,
            }],
            Source::Hosted(host) => self.hosted.candidates(name, host)?,
            Source::Sdk(sdk_name) => vec![super::sdk::candidate(
                self.sdk_root.as_deref(),
                sdk_name,
                name,
                &self.sdk,
                &self.hosted.default_host,
            )?],
            Source::Git {
                url,
                reference,
                path,
            } => {
                // An unchanged description keeps its locked commit, as Pub does.
                let locked = self
                    .locked
                    .get(name)
                    .filter(|(source, _)| *source == requirement.source)
                    .and_then(|_| self.locked_digests.get(name))
                    .map(String::as_str);
                vec![self.git.candidate(
                    name,
                    url,
                    reference,
                    path,
                    locked,
                    &self.hosted.default_host,
                )?]
            }
        };
        candidates.retain(|c| {
            c.manifest.name == *name
                && (self.overrides.contains_key(name) || check_sdk(&c.manifest, &self.sdk).is_ok())
                && requirement
                    .constraints
                    .iter()
                    .all(|(_, r)| r.allows(&c.manifest.version))
        });
        if let Some((source, version)) = self.locked.get(name)
            && source == &requirement.source
            && let Some(index) = candidates
                .iter()
                .position(|c| &c.manifest.version == version)
        {
            let candidate = candidates.remove(index);
            candidates.insert(0, candidate);
        }
        let mut error = conflict(name, requirement);
        for candidate in candidates {
            if let Some(error) = &candidate.manifest_error {
                return Err(SearchError::Fatal(format!(
                    "{} {}: {error}",
                    candidate.manifest.name, candidate.manifest.version
                )));
            }
            let mut next = requirements.clone();
            let mut dependencies = candidate.manifest.dependencies.clone();
            // Overridden development packages may refer back to the root with
            // their published version range and source; Pub ignores that edge.
            if self.overrides.contains_key(name) {
                dependencies.remove(&self.root_name);
            }
            if let Err(e) = add(
                &mut next,
                &dependencies,
                name,
                &self.overrides,
                &self.workspace,
            ) {
                error = e;
                continue;
            }
            let mut chosen = selected.clone();
            chosen.insert(
                name.clone(),
                Selected {
                    source: requirement.source.clone(),
                    candidate,
                    relative: requirement.relative,
                },
            );
            match self.search(next, chosen) {
                Ok(result) => return Ok(result),
                Err(SearchError::Conflict(e)) => error = e,
                Err(error) => return Err(error),
            }
        }
        Err(SearchError::Conflict(error))
    }
}
fn add(
    required: &mut BTreeMap<String, Required>,
    dependencies: &BTreeMap<String, Dependency>,
    parent: &str,
    overrides: &BTreeMap<String, Dependency>,
    workspace: &BTreeSet<String>,
) -> Result<(), String> {
    for (name, dependency) in dependencies {
        // Only workspace packages supply overrides; dependency pubspecs cannot alter the graph.
        let dependency = overrides.get(name).unwrap_or(dependency);
        let entry = required.entry(name.clone()).or_insert_with(|| Required {
            source: dependency.source.clone(),
            relative: dependency.relative,
            constraints: Vec::new(),
        });
        if entry.source != dependency.source && !workspace.contains(name) {
            return Err(format!(
                "conflicting package sources for {name}, required by {parent}"
            ));
        }
        entry
            .constraints
            .push((parent.into(), dependency.constraint.clone()));
    }
    Ok(())
}
fn conflict(name: &str, required: &Required) -> String {
    format!(
        "no compatible version for {name}: {}",
        required
            .constraints
            .iter()
            .map(|(parent, c)| format!("{parent} requires {}", c.text))
            .collect::<Vec<_>>()
            .join("; ")
    )
}
fn check_sdk(manifest: &Manifest, sdk: &Version) -> Result<(), String> {
    if sdk.major >= 3
        && manifest
            .sdk
            .lower
            .as_ref()
            .is_none_or(|(v, _)| (v.major, v.minor) < (2, 12))
    {
        return Err(format!(
            "{} does not declare a null-safe SDK lower bound (>=2.12.0)",
            manifest.name
        ));
    }
    if !manifest.sdk.for_sdk(sdk).allows(sdk) {
        return Err(format!(
            "{} requires Dart SDK {}, current SDK is {sdk}",
            manifest.name, manifest.sdk.text
        ));
    }
    Ok(())
}
