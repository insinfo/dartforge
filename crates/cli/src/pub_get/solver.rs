//! Pub 3.6.2's version solver: PubGrub, conflict-driven clause learning
//! over package version sets (`lib/src/solver/` of Pub).
//!
//! The port keeps Pub's decision heuristics exactly — fewest versions
//! first, locked versions preferred, newest stable versions first (oldest
//! when downgrading), facts learned in dependency order — because they
//! decide which of several valid solutions is found, and therefore the
//! lockfile.
use super::{
    manifest::{Dependency, Description, Manifest},
    source::{Cache, LockFile, PackageId, SDKS, SourceError},
    version::{Range, Version, VersionSet},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolveType {
    Get,
    Upgrade,
    Downgrade,
}

/// A workspace package: its pubspec, directory and direct members (as
/// indices into the workspace, which lists packages in Pub's
/// `transitiveWorkspace` order).
#[derive(Clone, Debug)]
pub(crate) struct WorkspacePackage {
    pub manifest: Manifest,
    pub directory: PathBuf,
    /// The directory as Pub spells it (`p.join(parent.dir, member)`).
    pub shown: String,
    pub children: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Relation {
    Subset,
    Disjoint,
    Overlapping,
}

/// A statement about a package (Pub's `Term`).
#[derive(Clone, Debug)]
pub(crate) struct Term {
    pub positive: bool,
    pub package: Dependency,
}

impl Term {
    fn new(package: Dependency, positive: bool) -> Self {
        Self { positive, package }
    }

    pub fn inverse(&self) -> Self {
        Self::new(self.package.clone(), !self.positive)
    }

    fn constraint(&self) -> &VersionSet {
        &self.package.constraint
    }

    pub fn is_root(&self) -> bool {
        self.package.description.is_root()
    }

    fn compatible(&self, other: &Dependency) -> bool {
        self.package.description.is_root()
            || other.description.is_root()
            || (other.name == self.package.name && other.description == self.package.description)
    }

    pub fn satisfies(&self, other: &Term) -> bool {
        self.package.name == other.package.name && self.relation(other) == Relation::Subset
    }

    fn relation(&self, other: &Term) -> Relation {
        let theirs = other.constraint();
        let ours = self.constraint();
        let compatible = self.compatible(&other.package);
        match (other.positive, self.positive) {
            (true, true) => {
                if !compatible {
                    Relation::Disjoint
                } else if theirs.allows_all(ours) {
                    Relation::Subset
                } else if !ours.allows_any(theirs) {
                    Relation::Disjoint
                } else {
                    Relation::Overlapping
                }
            }
            (true, false) => {
                if !compatible {
                    Relation::Overlapping
                } else if ours.allows_all(theirs) {
                    Relation::Disjoint
                } else {
                    Relation::Overlapping
                }
            }
            (false, true) => {
                if !compatible || !theirs.allows_any(ours) {
                    Relation::Subset
                } else if theirs.allows_all(ours) {
                    Relation::Disjoint
                } else {
                    Relation::Overlapping
                }
            }
            (false, false) => {
                if !compatible {
                    Relation::Overlapping
                } else if ours.allows_all(theirs) {
                    Relation::Subset
                } else {
                    Relation::Overlapping
                }
            }
        }
    }

    fn with(&self, constraint: VersionSet, positive: bool) -> Option<Term> {
        (!constraint.is_empty()).then(|| {
            Term::new(
                Dependency {
                    name: self.package.name.clone(),
                    description: self.package.description.clone(),
                    constraint,
                },
                positive,
            )
        })
    }

    pub fn intersect(&self, other: &Term) -> Option<Term> {
        if self.compatible(&other.package) {
            if self.positive != other.positive {
                let (positive, negative) = if self.positive {
                    (self, other)
                } else {
                    (other, self)
                };
                self.with(
                    positive.constraint().difference(negative.constraint()),
                    true,
                )
            } else if self.positive {
                self.with(self.constraint().intersect(other.constraint()), true)
            } else {
                self.with(self.constraint().union(other.constraint()), false)
            }
        } else if self.positive != other.positive {
            Some(if self.positive {
                self.clone()
            } else {
                other.clone()
            })
        } else {
            None
        }
    }

    fn difference(&self, other: &Term) -> Option<Term> {
        self.intersect(&other.inverse())
    }

    fn same_package(&self, other: &Dependency) -> bool {
        self.package == *other
    }
}

/// Why an incompatibility holds (Pub's `IncompatibilityCause`).
#[derive(Clone, Debug)]
pub(crate) enum Cause {
    Root,
    Dependency,
    NoVersions,
    UnknownSource,
    Sdk {
        constraint: Option<VersionSet>,
        sdk: String,
    },
    NotFound(String),
    Conflict(usize, usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Incompatibility {
    pub terms: Vec<Term>,
    pub cause: Cause,
}

impl Incompatibility {
    /// Pub's `Incompatibility` factory: drops the positive root term of a
    /// derived incompatibility and merges terms about the same package.
    fn new(mut terms: Vec<Term>, cause: Cause) -> Self {
        if terms.len() != 1
            && matches!(cause, Cause::Conflict(..))
            && terms.iter().any(|t| t.positive && t.is_root())
        {
            terms.retain(|t| !t.positive || !t.is_root());
        }
        if terms.len() == 1 || (terms.len() == 2 && terms[0].package.name != terms[1].package.name)
        {
            return Self { terms, cause };
        }
        let mut by_name: Vec<(String, Vec<Term>)> = Vec::new();
        for term in terms {
            let index = match by_name.iter().position(|(n, _)| *n == term.package.name) {
                Some(index) => index,
                None => {
                    by_name.push((term.package.name.clone(), Vec::new()));
                    by_name.len() - 1
                }
            };
            let by_ref = &mut by_name[index].1;
            match by_ref
                .iter_mut()
                .find(|t| t.package.description == term.package.description)
            {
                Some(existing) => {
                    *existing = existing
                        .intersect(&term)
                        .expect("terms of an incompatibility intersect");
                }
                None => by_ref.push(term),
            }
        }
        let mut result = Vec::new();
        for (_, by_ref) in by_name {
            if by_ref.iter().any(|t| t.positive) {
                result.extend(by_ref.into_iter().filter(|t| t.positive));
            } else {
                result.extend(by_ref);
            }
        }
        Self {
            terms: result,
            cause,
        }
    }

    pub fn is_failure(&self) -> bool {
        self.terms.is_empty() || (self.terms.len() == 1 && self.terms[0].is_root())
    }
}

struct Assignment {
    term: Term,
    level: usize,
    index: usize,
    cause: Option<usize>,
}

/// Pub's `PartialSolution`; `positive` keeps insertion order like Dart's
/// `LinkedHashMap`, since it decides the order of `unsatisfied`.
#[derive(Default)]
struct PartialSolution {
    assignments: Vec<Assignment>,
    decisions: Vec<PackageId>,
    decided: HashSet<String>,
    positive_order: Vec<String>,
    positive: HashMap<String, Term>,
    negative: HashMap<String, Vec<Term>>,
    attempted: usize,
    backtracking: bool,
}

impl PartialSolution {
    fn level(&self) -> usize {
        self.decisions.len()
    }

    fn unsatisfied(&self) -> Vec<Dependency> {
        self.positive_order
            .iter()
            .filter(|name| !self.decided.contains(*name))
            .map(|name| self.positive[name].package.clone())
            .collect()
    }

    fn decide(&mut self, id: PackageId) {
        if self.backtracking {
            self.attempted += 1;
        }
        self.backtracking = false;
        let term = Term::new(id.to_range(), true);
        self.decided.insert(id.name.clone());
        self.decisions.push(id);
        let assignment = Assignment {
            term,
            level: self.level(),
            index: self.assignments.len(),
            cause: None,
        };
        self.assign(assignment);
    }

    fn derive(&mut self, package: Dependency, positive: bool, cause: usize) {
        let assignment = Assignment {
            term: Term::new(package, positive),
            level: self.level(),
            index: self.assignments.len(),
            cause: Some(cause),
        };
        self.assign(assignment);
    }

    fn assign(&mut self, assignment: Assignment) {
        let term = assignment.term.clone();
        self.assignments.push(assignment);
        self.register(&term);
    }

    fn backtrack(&mut self, level: usize) {
        self.backtracking = true;
        let mut packages = HashSet::new();
        while self.assignments.last().is_some_and(|a| a.level > level) {
            let removed = self.assignments.pop().expect("checked");
            packages.insert(removed.term.package.name.clone());
            if removed.cause.is_none() {
                let decision = self.decisions.pop().expect("a decision was removed");
                self.decided.remove(&decision.name);
            }
        }
        for package in &packages {
            self.positive.remove(package);
            self.negative.remove(package);
        }
        self.positive_order.retain(|name| !packages.contains(name));
        let terms: Vec<Term> = self
            .assignments
            .iter()
            .filter(|a| packages.contains(&a.term.package.name))
            .map(|a| a.term.clone())
            .collect();
        for term in terms {
            self.register(&term);
        }
    }

    fn register(&mut self, term: &Term) {
        let name = &term.package.name;
        if let Some(old) = self.positive.get(name) {
            let merged = old
                .intersect(term)
                .expect("a positive assignment stays satisfiable");
            self.positive.insert(name.clone(), merged);
            return;
        }
        let negatives = self.negative.entry(name.clone()).or_default();
        let index = negatives
            .iter()
            .position(|t| t.package.description == term.package.description);
        let merged = match index {
            Some(index) => term
                .intersect(&negatives[index])
                .expect("a negative assignment stays satisfiable"),
            None => term.clone(),
        };
        if merged.positive {
            self.negative.remove(name);
            self.positive.insert(name.clone(), merged);
            self.positive_order.push(name.clone());
            return;
        }
        let negatives = self.negative.entry(name.clone()).or_default();
        match index {
            Some(index) => negatives[index] = merged,
            None => negatives.push(merged),
        }
    }

    fn satisfier(&self, term: &Term) -> &Assignment {
        let mut assigned: Option<Term> = None;
        for assignment in &self.assignments {
            if assignment.term.package.name != term.package.name {
                continue;
            }
            if !assignment.term.is_root()
                && assignment.term.package.description != term.package.description
            {
                if !assignment.term.positive {
                    continue;
                }
                return assignment;
            }
            assigned = Some(match assigned {
                None => assignment.term.clone(),
                Some(current) => current
                    .intersect(&assignment.term)
                    .expect("assignments of a package intersect"),
            });
            if assigned.as_ref().is_some_and(|a| a.satisfies(term)) {
                return assignment;
            }
        }
        panic!("[BUG] {} is not satisfied.", term.package.name);
    }

    fn satisfies(&self, term: &Term) -> bool {
        self.relation(term) == Relation::Subset
    }

    fn relation(&self, term: &Term) -> Relation {
        if let Some(positive) = self.positive.get(&term.package.name) {
            return positive.relation(term);
        }
        self.negative
            .get(&term.package.name)
            .and_then(|by_ref| {
                by_ref
                    .iter()
                    .find(|t| t.package.description == term.package.description)
            })
            .map(|negative| negative.relation(term))
            .unwrap_or(Relation::Overlapping)
    }
}

/// Pub's `PackageLister` for one package reference.
struct Lister {
    name: String,
    description: Description,
    /// The workspace package this lister describes, if it is one.
    root: Option<usize>,
    locked: Option<PackageId>,
    allowed_retracted: Option<Version>,
    overridden: HashSet<String>,
    downgrade: bool,
    already_listed: HashMap<String, VersionSet>,
    known_invalid: VersionSet,
    listed_locked: bool,
    versions: Option<Result<Vec<PackageId>, String>>,
}

/// How a solve ended without a solution.
pub(crate) enum SolveError {
    /// Pub's `SolveFailure`: the incompatibility that proves no solution
    /// exists, with all the facts it was derived from.
    Failure {
        incompatibilities: Vec<Incompatibility>,
        root: usize,
    },
    Fatal(String),
}

impl From<String> for SolveError {
    fn from(error: String) -> Self {
        Self::Fatal(error)
    }
}

pub(crate) struct SolveResult {
    /// The decisions, workspace packages included, in decision order.
    pub packages: Vec<PackageId>,
    pub pubspecs: HashMap<String, Rc<Manifest>>,
    /// The versions known for each selected package (for the report).
    pub available: HashMap<String, Vec<Version>>,
    /// Pub's `attemptedSolutions`.
    #[allow(dead_code)]
    pub attempted: usize,
}

pub(crate) struct Solver<'a> {
    cache: &'a mut Cache,
    kind: SolveType,
    workspace: &'a [WorkspacePackage],
    lock: &'a LockFile,
    unlock: HashSet<String>,
    overrides: Vec<Dependency>,
    overridden: HashSet<String>,
    incompatibilities: Vec<Incompatibility>,
    by_package: HashMap<String, Vec<usize>>,
    solution: PartialSolution,
    listers: Vec<Lister>,
    lister_index: HashMap<(String, Description), usize>,
}

enum Propagation {
    None,
    Conflict,
    Derived(String),
}

impl<'a> Solver<'a> {
    pub fn new(
        kind: SolveType,
        cache: &'a mut Cache,
        workspace: &'a [WorkspacePackage],
        lock: &'a LockFile,
        unlock: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut overrides: Vec<Dependency> = Vec::new();
        for package in workspace {
            for dependency in &package.manifest.overrides {
                match overrides.iter_mut().find(|d| d.name == dependency.name) {
                    Some(existing) => *existing = dependency.clone(),
                    None => overrides.push(dependency.clone()),
                }
            }
        }
        let overridden = overrides.iter().map(|d| d.name.clone()).collect();
        Self {
            cache,
            kind,
            workspace,
            lock,
            unlock: unlock.into_iter().collect(),
            overrides,
            overridden,
            incompatibilities: Vec::new(),
            by_package: HashMap::new(),
            solution: PartialSolution {
                attempted: 1,
                ..PartialSolution::default()
            },
            listers: Vec::new(),
            lister_index: HashMap::new(),
        }
    }

    fn root_range(&self, index: usize) -> Dependency {
        let package = &self.workspace[index];
        Dependency {
            name: package.manifest.name.clone(),
            description: Description::Root(package.directory.clone()),
            constraint: VersionSet::exact(&package.manifest.version),
        }
    }

    pub fn solve(mut self) -> Result<SolveResult, SolveError> {
        let root = self.root_range(0);
        self.add(Incompatibility::new(
            vec![Term::new(root.clone(), false)],
            Cause::Root,
        ));
        let mut next = Some(root.name);
        while let Some(package) = next {
            self.propagate(&package)?;
            next = self.choose_package_version()?;
        }
        self.result()
    }

    fn add(&mut self, incompatibility: Incompatibility) -> usize {
        let id = self.incompatibilities.len();
        for term in &incompatibility.terms {
            self.by_package
                .entry(term.package.name.clone())
                .or_default()
                .push(id);
        }
        self.incompatibilities.push(incompatibility);
        id
    }

    fn propagate(&mut self, package: &str) -> Result<(), SolveError> {
        let mut changed: Vec<String> = vec![package.to_owned()];
        while !changed.is_empty() {
            let package = changed.remove(0);
            let ids: Vec<usize> = self.by_package.get(&package).cloned().unwrap_or_default();
            for &id in ids.iter().rev() {
                match self.propagate_incompatibility(id) {
                    Propagation::Conflict => {
                        let root_cause = self.resolve_conflict(id)?;
                        changed.clear();
                        match self.propagate_incompatibility(root_cause) {
                            Propagation::Derived(name) => changed.push(name),
                            _ => {
                                return Err(SolveError::Fatal(
                                    "[BUG] the root cause of a conflict did not propagate".into(),
                                ));
                            }
                        }
                        break;
                    }
                    Propagation::Derived(name) => {
                        if !changed.contains(&name) {
                            changed.push(name);
                        }
                    }
                    Propagation::None => {}
                }
            }
        }
        Ok(())
    }

    fn propagate_incompatibility(&mut self, id: usize) -> Propagation {
        let mut unsatisfied: Option<usize> = None;
        for (index, term) in self.incompatibilities[id].terms.iter().enumerate() {
            match self.solution.relation(term) {
                Relation::Disjoint => return Propagation::None,
                Relation::Overlapping => {
                    if unsatisfied.is_some() {
                        return Propagation::None;
                    }
                    unsatisfied = Some(index);
                }
                Relation::Subset => {}
            }
        }
        let Some(index) = unsatisfied else {
            return Propagation::Conflict;
        };
        let term = self.incompatibilities[id].terms[index].clone();
        let name = term.package.name.clone();
        self.solution.derive(term.package, !term.positive, id);
        Propagation::Derived(name)
    }

    fn resolve_conflict(&mut self, mut id: usize) -> Result<usize, SolveError> {
        let mut new_incompatibility = false;
        while !self.incompatibilities[id].is_failure() {
            let incompatibility = self.incompatibilities[id].clone();
            let mut most_recent_term: Option<usize> = None;
            let mut most_recent_satisfier: Option<(usize, usize, Option<usize>, Dependency)> = None;
            let mut difference: Option<Term> = None;
            let mut previous_level = 1;
            for (index, term) in incompatibility.terms.iter().enumerate() {
                let satisfier = self.solution.satisfier(term);
                let info = (
                    satisfier.index,
                    satisfier.level,
                    satisfier.cause,
                    satisfier.term.package.clone(),
                );
                let satisfier_term = satisfier.term.clone();
                match &most_recent_satisfier {
                    None => {
                        most_recent_term = Some(index);
                        most_recent_satisfier = Some(info);
                    }
                    Some(current) if current.0 < info.0 => {
                        previous_level = previous_level.max(current.1);
                        most_recent_term = Some(index);
                        most_recent_satisfier = Some(info);
                        difference = None;
                    }
                    Some(_) => {
                        previous_level = previous_level.max(info.1);
                    }
                }
                if most_recent_term == Some(index) {
                    difference = satisfier_term.difference(term);
                    if let Some(difference) = &difference {
                        previous_level = previous_level
                            .max(self.solution.satisfier(&difference.inverse()).level);
                    }
                }
            }
            let (_, level, cause, satisfier_package) =
                most_recent_satisfier.expect("an incompatibility has terms");
            if previous_level < level || cause.is_none() {
                self.solution.backtrack(previous_level);
                if new_incompatibility {
                    let terms = self.incompatibilities[id].terms.clone();
                    for term in &terms {
                        self.by_package
                            .entry(term.package.name.clone())
                            .or_default()
                            .push(id);
                    }
                }
                return Ok(id);
            }
            let cause = cause.expect("checked");
            let mut terms: Vec<Term> = incompatibility
                .terms
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != most_recent_term)
                .map(|(_, term)| term.clone())
                .collect();
            terms.extend(
                self.incompatibilities[cause]
                    .terms
                    .iter()
                    .filter(|term| !term.same_package(&satisfier_package))
                    .cloned(),
            );
            if let Some(difference) = &difference {
                terms.push(difference.inverse());
            }
            let derived = Incompatibility::new(terms, Cause::Conflict(id, cause));
            // Learned facts are registered when the backjump happens.
            id = self.incompatibilities.len();
            self.incompatibilities.push(derived);
            new_incompatibility = true;
        }
        Err(SolveError::Failure {
            incompatibilities: std::mem::take(&mut self.incompatibilities),
            root: id,
        })
    }

    fn lister(&mut self, package: &Dependency) -> usize {
        let key = (package.name.clone(), package.description.clone());
        if let Some(&index) = self.lister_index.get(&key) {
            return index;
        }
        let lister = if let Description::Root(directory) = &package.description {
            let root = self
                .workspace
                .iter()
                .position(|p| p.manifest.name == package.name && p.directory == *directory);
            let version = root
                .map(|r| self.workspace[r].manifest.version.clone())
                .unwrap_or_else(|| Version::new(0, 0, 0));
            Lister {
                name: package.name.clone(),
                description: package.description.clone(),
                root,
                locked: Some(PackageId {
                    name: package.name.clone(),
                    version,
                    description: package.description.clone(),
                    resolved: None,
                }),
                allowed_retracted: None,
                overridden: self.overridden.clone(),
                downgrade: false,
                already_listed: HashMap::new(),
                known_invalid: VersionSet::empty(),
                listed_locked: false,
                versions: None,
            }
        } else {
            let locked = self
                .locked(&package.name)
                .filter(|l| l.description == package.description)
                .cloned();
            let mut overridden = self.overridden.clone();
            if self.overridden.contains(&package.name) {
                for root in self.workspace {
                    overridden.insert(root.manifest.name.clone());
                }
            }
            let allowed_retracted = match self.overrides.iter().find(|d| d.name == package.name) {
                Some(dependency) if dependency.constraint.as_version().is_some() => {
                    dependency.constraint.as_version().cloned()
                }
                _ => self
                    .lock
                    .packages
                    .get(&package.name)
                    .map(|id| id.version.clone()),
            };
            Lister {
                name: package.name.clone(),
                description: package.description.clone(),
                root: None,
                locked,
                allowed_retracted,
                overridden,
                downgrade: self.kind == SolveType::Downgrade,
                already_listed: HashMap::new(),
                known_invalid: VersionSet::empty(),
                listed_locked: false,
                versions: None,
            }
        };
        self.listers.push(lister);
        let index = self.listers.len() - 1;
        self.lister_index.insert(key, index);
        index
    }

    /// Pub's `_getLocked`.
    fn locked(&self, name: &str) -> Option<&PackageId> {
        match self.kind {
            SolveType::Get => {
                if self.unlock.contains(name) {
                    return None;
                }
                return self.lock.packages.get(name);
            }
            SolveType::Downgrade => {
                if let Some(locked) = self.lock.packages.get(name)
                    && !locked.description.has_multiple_versions()
                {
                    return Some(locked);
                }
            }
            SolveType::Upgrade => {}
        }
        if self.unlock.is_empty() || self.unlock.contains(name) {
            return None;
        }
        self.lock.packages.get(name)
    }

    /// The sorted versions of a lister's package, fetched once.
    fn versions(&mut self, lister: usize) -> Result<Result<&Vec<PackageId>, String>, String> {
        if self.listers[lister].versions.is_none() {
            let l = &self.listers[lister];
            let result = if let Some(root) = l.root {
                Ok(vec![PackageId {
                    name: l.name.clone(),
                    version: self.workspace[root].manifest.version.clone(),
                    description: l.description.clone(),
                    resolved: None,
                }])
            } else {
                let (name, description, retracted) = (
                    l.name.clone(),
                    l.description.clone(),
                    l.allowed_retracted.clone(),
                );
                match self.cache.versions(&name, &description, retracted.as_ref()) {
                    Ok(mut versions) => {
                        versions.sort_by(|a, b| a.version.cmp(&b.version));
                        Ok(versions)
                    }
                    Err(SourceError::NotFound(message)) => Err(message),
                    Err(SourceError::Invalid(message)) | Err(SourceError::Fatal(message)) => {
                        return Err(message);
                    }
                }
            };
            self.listers[lister].versions = Some(result);
        }
        Ok(self.listers[lister]
            .versions
            .as_ref()
            .expect("set above")
            .as_ref()
            .map_err(Clone::clone))
    }

    fn count_versions(&mut self, lister: usize, constraint: &VersionSet) -> Result<usize, String> {
        if let Some(locked) = &self.listers[lister].locked
            && constraint.allows(&locked.version)
        {
            return Ok(1);
        }
        Ok(match self.versions(lister)? {
            Ok(versions) => versions
                .iter()
                .filter(|id| constraint.allows(&id.version))
                .count(),
            Err(_) => 0,
        })
    }

    /// `Ok(Err(message))` when the package does not exist.
    fn best_version(
        &mut self,
        lister: usize,
        constraint: &VersionSet,
    ) -> Result<Result<Option<PackageId>, String>, String> {
        if let Some(locked) = &self.listers[lister].locked
            && constraint.allows(&locked.version)
        {
            return Ok(Ok(Some(locked.clone())));
        }
        let downgrade = self.listers[lister].downgrade;
        let versions = match self.versions(lister)? {
            Ok(versions) => versions,
            Err(message) => return Ok(Err(message)),
        };
        let mut best_prerelease = None;
        let ordered: Box<dyn Iterator<Item = &PackageId>> = if downgrade {
            Box::new(versions.iter())
        } else {
            Box::new(versions.iter().rev())
        };
        for id in ordered {
            if !constraint.allows(&id.version) {
                continue;
            }
            if !id.version.is_prerelease() {
                return Ok(Ok(Some(id.clone())));
            }
            if best_prerelease.is_none() {
                best_prerelease = Some(id.clone());
            }
        }
        Ok(Ok(best_prerelease))
    }

    fn describe_safe(&mut self, id: &PackageId) -> Rc<Manifest> {
        match self.cache.describe(id) {
            Ok(manifest) => manifest,
            Err(_) => Rc::new(Manifest::empty(&id.name, id.version.clone())),
        }
    }

    fn matches_sdk(&self, lister: usize, manifest: &Manifest, sdk: &str) -> bool {
        if self.listers[lister].overridden.contains(&manifest.name) {
            return true;
        }
        let Some((_, constraint)) = manifest.sdks.iter().find(|(name, _)| name == sdk) else {
            return true;
        };
        match self.cache.sdk_version(sdk) {
            Some(version) => constraint.effective.allows(&version),
            None => false,
        }
    }

    fn dependency_incompatibility(depender: Dependency, target: Dependency) -> Incompatibility {
        Incompatibility::new(
            vec![Term::new(depender, true), Term::new(target, false)],
            Cause::Dependency,
        )
    }

    /// Pub's `PackageLister.incompatibilitiesFor`.
    fn incompatibilities_for(
        &mut self,
        lister: usize,
        id: &PackageId,
    ) -> Result<Vec<Incompatibility>, String> {
        if self.listers[lister].known_invalid.allows(&id.version) {
            return Ok(Vec::new());
        }
        let manifest = match self.listers[lister].root {
            Some(root) => Rc::new(self.workspace[root].manifest.clone()),
            None => match self.cache.describe(id) {
                Ok(manifest) => manifest,
                Err(SourceError::Invalid(_)) | Err(SourceError::NotFound(_)) => {
                    let l = &mut self.listers[lister];
                    l.known_invalid = l.known_invalid.union(&VersionSet::exact(&id.version));
                    return Ok(vec![Incompatibility::new(
                        vec![Term::new(id.to_range(), true)],
                        Cause::NoVersions,
                    )]);
                }
                Err(SourceError::Fatal(message)) => return Err(message),
            },
        };
        if let Some(error) = &manifest.error {
            return Err(format!("{} {}: {error}", id.name, id.version));
        }
        let l = &self.listers[lister];
        if l.versions.is_none()
            && l.locked
                .as_ref()
                .is_some_and(|locked| locked.version == id.version)
        {
            if l.listed_locked {
                return Ok(Vec::new());
            }
            let depender = id.to_range();
            self.listers[lister].listed_locked = true;
            for sdk in SDKS {
                if !self.matches_sdk(lister, &manifest, sdk) {
                    let constraint = manifest
                        .sdks
                        .iter()
                        .find(|(name, _)| name == sdk)
                        .map(|(_, c)| c.effective.clone());
                    return Ok(vec![Incompatibility::new(
                        vec![Term::new(depender, true)],
                        Cause::Sdk {
                            constraint,
                            sdk: sdk.to_owned(),
                        },
                    )]);
                }
            }
            let overridden = &self.listers[lister].overridden;
            let mut entries: Vec<Dependency> = manifest
                .dependencies
                .iter()
                .filter(|d| !overridden.contains(&d.name))
                .cloned()
                .collect();
            if let Some(root) = self.listers[lister].root {
                entries.extend(
                    manifest
                        .dev
                        .iter()
                        .filter(|d| !overridden.contains(&d.name))
                        .cloned(),
                );
                for &child in &self.workspace[root].children {
                    entries.push(self.root_range(child));
                }
                entries.extend(manifest.overrides.iter().cloned());
            }
            return Ok(entries
                .into_iter()
                .map(|target| Self::dependency_incompatibility(depender.clone(), target))
                .collect());
        }
        let versions = self.versions(lister)??.clone();
        let index = versions.partition_point(|v| v.version < id.version);
        if index >= versions.len() || versions[index].version != id.version {
            return Err(format!("[BUG] {} {} is not listed", id.name, id.version));
        }
        for sdk in SDKS {
            if let Some(incompatibility) = self.check_sdk_constraint(lister, &versions, index, sdk)
            {
                return Ok(vec![incompatibility]);
            }
        }
        let mut dependencies: Vec<Dependency> = Vec::new();
        for dependency in &manifest.dependencies {
            let l = &self.listers[lister];
            if l.overridden.contains(&dependency.name) {
                continue;
            }
            if l.already_listed
                .get(&dependency.name)
                .is_some_and(|c| c.allows(&id.version))
            {
                continue;
            }
            dependencies.push(dependency.clone());
        }
        let lower = self.dependency_bounds(lister, &versions, &dependencies, index, false);
        let upper = self.dependency_bounds(lister, &versions, &dependencies, index, true);
        dependencies.sort_by(|a, b| a.name.cmp(&b.name));
        let mut result = Vec::with_capacity(dependencies.len());
        for dependency in dependencies {
            let constraint = VersionSet::from_range(Range {
                min: lower.get(&dependency.name).cloned(),
                include_min: true,
                max: upper.get(&dependency.name).cloned(),
                include_max: false,
            });
            let l = &mut self.listers[lister];
            let listed = l
                .already_listed
                .get(&dependency.name)
                .map(|c| c.union(&constraint))
                .unwrap_or_else(|| constraint.clone());
            l.already_listed.insert(dependency.name.clone(), listed);
            let depender = Dependency {
                name: l.name.clone(),
                description: l.description.clone(),
                constraint,
            };
            result.push(Self::dependency_incompatibility(depender, dependency));
        }
        Ok(result)
    }

    fn check_sdk_constraint(
        &mut self,
        lister: usize,
        versions: &[PackageId],
        index: usize,
        sdk: &str,
    ) -> Option<Incompatibility> {
        let manifest = self.describe_safe(&versions[index]);
        if self.matches_sdk(lister, &manifest, sdk) {
            return None;
        }
        let mut first = index as i64 - 1;
        while first > 0 {
            let manifest = self.describe_safe(&versions[first as usize]);
            if self.matches_sdk(lister, &manifest, sdk) {
                break;
            }
            first -= 1;
        }
        let mut last = index + 1;
        while last < versions.len() {
            let manifest = self.describe_safe(&versions[last]);
            if self.matches_sdk(lister, &manifest, sdk) {
                break;
            }
            last += 1;
        }
        let (first, last) = ((first + 1) as usize, last - 1);
        let incompatible = VersionSet::from_range(Range {
            min: (first != 0).then(|| versions[first].version.clone()),
            include_min: true,
            max: (last != versions.len() - 1).then(|| versions[last + 1].version.clone()),
            include_max: false,
        });
        let l = &mut self.listers[lister];
        l.known_invalid = incompatible.union(&l.known_invalid);
        let mut constraint = VersionSet::empty();
        for id in &versions[first..=last] {
            let manifest = self.describe_safe(id);
            let effective = manifest
                .sdks
                .iter()
                .find(|(name, _)| name == sdk)
                .map(|(_, c)| c.effective.clone())
                .unwrap_or_else(VersionSet::any);
            constraint = constraint.union(&effective);
        }
        let l = &self.listers[lister];
        Some(Incompatibility::new(
            vec![Term::new(
                Dependency {
                    name: l.name.clone(),
                    description: l.description.clone(),
                    constraint: incompatible,
                },
                true,
            )],
            Cause::Sdk {
                constraint: Some(constraint),
                sdk: sdk.to_owned(),
            },
        ))
    }

    /// Pub's `_dependencyBounds`: around `index`, how far the same
    /// dependency on each package extends.
    fn dependency_bounds(
        &mut self,
        lister: usize,
        versions: &[PackageId],
        dependencies: &[Dependency],
        index: usize,
        upper: bool,
    ) -> HashMap<String, Version> {
        let mut bounds: HashMap<String, Version> = HashMap::new();
        let mut previous = versions[index].version.clone();
        let candidates: Vec<&PackageId> = if upper {
            versions[index + 1..].iter().collect()
        } else {
            versions[..index].iter().rev().collect()
        };
        'outer: for id in candidates {
            let manifest = self.describe_safe(id);
            let boundary = if upper {
                id.version.clone()
            } else {
                previous.clone()
            };
            for sdk in SDKS {
                if self.matches_sdk(lister, &manifest, sdk) {
                    continue;
                }
                for dependency in dependencies {
                    bounds
                        .entry(dependency.name.clone())
                        .or_insert_with(|| boundary.clone());
                }
                break 'outer;
            }
            for dependency in dependencies {
                if bounds.contains_key(&dependency.name) {
                    continue;
                }
                if manifest.dependency(&dependency.name) != Some(dependency) {
                    bounds.insert(dependency.name.clone(), boundary.clone());
                }
            }
            if bounds.len() == dependencies.len() {
                break;
            }
            previous = id.version.clone();
        }
        bounds
    }

    fn choose_package_version(&mut self) -> Result<Option<String>, SolveError> {
        let unsatisfied = self.solution.unsatisfied();
        if unsatisfied.is_empty() {
            return Ok(None);
        }
        if let Some(candidate) = unsatisfied
            .iter()
            .find(|c| matches!(c.description, Description::Unknown { .. }))
        {
            let name = candidate.name.clone();
            self.add(Incompatibility::new(
                vec![Term::new(
                    Dependency {
                        constraint: VersionSet::any(),
                        ..candidate.clone()
                    },
                    true,
                )],
                Cause::UnknownSource,
            ));
            return Ok(Some(name));
        }
        let mut best: Option<(usize, &Dependency)> = None;
        for candidate in &unsatisfied {
            let lister = self.lister(candidate);
            let count = self.count_versions(lister, &candidate.constraint)?;
            if best.is_none_or(|(c, _)| count < c) {
                best = Some((count, candidate));
            }
        }
        let package = best.expect("unsatisfied is not empty").1.clone();
        let lister = self.lister(&package);
        let mut version = match self.best_version(lister, &package.constraint)? {
            Ok(version) => version,
            Err(message) => {
                self.add(Incompatibility::new(
                    vec![Term::new(
                        Dependency {
                            constraint: VersionSet::any(),
                            ..package.clone()
                        },
                        true,
                    )],
                    Cause::NotFound(message),
                ));
                return Ok(Some(package.name));
            }
        };
        if version.is_none() {
            if VersionSet::any()
                .difference(&package.constraint)
                .as_version()
                .is_some()
            {
                version = match self.best_version(lister, &VersionSet::any())? {
                    Ok(version) => version,
                    Err(message) => return Err(SolveError::Fatal(message)),
                };
            } else {
                self.add(Incompatibility::new(
                    vec![Term::new(package.clone(), true)],
                    Cause::NoVersions,
                ));
                return Ok(Some(package.name));
            }
        }
        let version = version.ok_or_else(|| {
            SolveError::Fatal(format!("no versions of {} are available", package.name))
        })?;
        let mut conflict = false;
        for incompatibility in self.incompatibilities_for(lister, &version)? {
            conflict = conflict
                || incompatibility
                    .terms
                    .iter()
                    .all(|t| t.package.name == package.name || self.solution.satisfies(t));
            self.add(incompatibility);
        }
        if !conflict {
            self.solution.decide(version);
        }
        Ok(Some(package.name))
    }

    fn result(self) -> Result<SolveResult, SolveError> {
        let decisions = self.solution.decisions.clone();
        let mut pubspecs = HashMap::new();
        let mut available = HashMap::new();
        for id in &decisions {
            let manifest = match &id.description {
                Description::Root(directory) => Rc::new(
                    self.workspace
                        .iter()
                        .find(|p| p.manifest.name == id.name && p.directory == *directory)
                        .map(|p| p.manifest.clone())
                        .ok_or_else(|| format!("[BUG] unknown workspace package {}", id.name))?,
                ),
                _ => self.cache.describe(id).map_err(|e| match e {
                    SourceError::NotFound(m) | SourceError::Invalid(m) | SourceError::Fatal(m) => {
                        SolveError::Fatal(m)
                    }
                })?,
            };
            pubspecs.insert(id.name.clone(), manifest);
            let versions = match &id.description {
                Description::Hosted(_) => self
                    .cache
                    .versions(&id.name, &id.description, None)
                    .map(|ids| ids.into_iter().map(|i| i.version).collect())
                    .unwrap_or_else(|_| vec![id.version.clone()]),
                _ => vec![id.version.clone()],
            };
            available.insert(id.name.clone(), versions);
        }
        Ok(SolveResult {
            packages: decisions,
            pubspecs,
            available,
            attempted: self.solution.attempted,
        })
    }
}
