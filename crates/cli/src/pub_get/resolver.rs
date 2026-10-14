//! Deterministic dependency search with lockfile preferences and backtracking.
use super::{
    constraints::Constraint,
    hosted::{Candidate, Hosted},
    manifest::{Dependency, Manifest, Source},
};
use semver::Version;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct Selected {
    pub source: Source,
    pub candidate: Candidate,
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
    constraints: Vec<(String, Constraint)>,
}
pub(super) struct Resolver {
    pub hosted: Hosted,
    pub sdk: Version,
    pub locked: BTreeMap<String, (Source, Version)>,
    attempts: usize,
}
impl Resolver {
    pub fn new(hosted: Hosted, sdk: Version, locked: BTreeMap<String, (Source, Version)>) -> Self {
        Self {
            hosted,
            sdk,
            locked,
            attempts: 0,
        }
    }
    pub fn resolve(
        &mut self,
        root: &Manifest,
        directory: &std::path::Path,
    ) -> Result<BTreeMap<String, Selected>, String> {
        check_sdk(root, &self.sdk)?;
        let mut requirements = BTreeMap::new();
        add(&mut requirements, &root.dependencies, &root.name)?;
        add(&mut requirements, &root.dev, &root.name)?;
        let root_candidate = Candidate {
            manifest: root.clone(),
            manifest_error: None,
            directory: directory.into(),
            archive: None,
            digest: None,
        };
        let mut selected = BTreeMap::new();
        selected.insert(
            root.name.clone(),
            Selected {
                source: Source::Path(directory.into()),
                candidate: root_candidate,
            },
        );
        let mut result = self.search(requirements, selected).map_err(|e| match e {
            SearchError::Conflict(s) | SearchError::Fatal(s) => s,
        })?;
        result.remove(&root.name);
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
            if let Some(value) = selected.get(name) {
                if value.source != requirement.source
                    || !requirement
                        .constraints
                        .iter()
                        .all(|(_, c)| c.allows(&value.candidate.manifest.version))
                {
                    return Err(SearchError::Conflict(conflict(name, requirement)));
                }
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
        };
        candidates.retain(|c| {
            c.manifest.name == *name
                && check_sdk(&c.manifest, &self.sdk).is_ok()
                && requirement
                    .constraints
                    .iter()
                    .all(|(_, r)| r.allows(&c.manifest.version))
        });
        if let Some((source, version)) = self.locked.get(name) {
            if source == &requirement.source {
                if let Some(index) = candidates
                    .iter()
                    .position(|c| &c.manifest.version == version)
                {
                    let candidate = candidates.remove(index);
                    candidates.insert(0, candidate);
                }
            }
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
            if let Err(e) = add(&mut next, &candidate.manifest.dependencies, name) {
                error = e;
                continue;
            }
            let mut chosen = selected.clone();
            chosen.insert(
                name.clone(),
                Selected {
                    source: requirement.source.clone(),
                    candidate,
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
) -> Result<(), String> {
    for (name, dependency) in dependencies {
        let entry = required.entry(name.clone()).or_insert_with(|| Required {
            source: dependency.source.clone(),
            constraints: Vec::new(),
        });
        if entry.source != dependency.source {
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
