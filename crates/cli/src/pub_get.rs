//! `dartforge pub`: package resolution and the everyday `pub` commands of
//! Dart 3.6.2, implemented natively (no Dart or Pub subprocess).
//!
//! The solver is a port of Pub's PubGrub solver, so the same inputs give
//! the same lockfile; `pubspec.lock` and `.dart_tool/package_config.json`
//! are written in Pub's exact layout (`generator: pub`), so tools that
//! check them accept the result.
mod commands;
mod edit;
mod failure;
mod git;
mod hosted;
mod inspect;
mod manifest;
mod outdated;
mod output;
mod report;
mod sdk;
mod solver;
mod source;
#[cfg(test)]
mod tests;
mod tree;
mod version;
mod yaml_edit;
use manifest::{Context, Description, Manifest, Resolution, Role, normalize};
use output::NewLock;
use solver::{SolveError, SolveType, Solver, WorkspacePackage};
use source::{Cache, LockFile};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ffi::OsString,
    path::{Path, PathBuf},
};
use version::{Version, VersionSet};

pub(crate) const USAGE: &str = "Usage: dartforge pub <subcommand> [arguments...]

Available subcommands:
  add         Add dependencies to `pubspec.yaml`.
  deps        Print package dependencies.
  downgrade   Downgrade packages in a project.
  get         Get the current package's dependencies.
  outdated    Analyze dependencies to find which ones can be upgraded.
  remove      Removes dependencies from `pubspec.yaml`.
  upgrade     Upgrade the current package's dependencies to latest versions.";

/// A failure with the exit code `dart pub` uses for it.
#[derive(Debug)]
pub(crate) struct Failure {
    pub code: u8,
    pub message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self { code: 1, message }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

pub(crate) mod exit {
    pub const USAGE: u8 = 64;
    pub const DATA: u8 = 65;
    pub const NO_INPUT: u8 = 66;
    pub const UNAVAILABLE: u8 = 69;
}

pub(crate) fn usage(message: impl Into<String>) -> Failure {
    Failure {
        code: exit::USAGE,
        message: message.into(),
    }
}

pub(crate) fn data(message: impl Into<String>) -> Failure {
    Failure {
        code: exit::DATA,
        message: message.into(),
    }
}

/// Command-line arguments of one subcommand: flags, options and the rest.
pub(crate) struct Arguments {
    flags: BTreeMap<String, bool>,
    options: BTreeMap<String, Vec<String>>,
    pub rest: Vec<String>,
}

/// One option a subcommand accepts: its name, short form, whether it takes
/// a value, and whether `--no-` is accepted.
pub(crate) struct Spec {
    pub name: &'static str,
    pub abbr: Option<char>,
    pub value: bool,
    pub negatable: bool,
}

pub(crate) const fn flag(name: &'static str, abbr: Option<char>, negatable: bool) -> Spec {
    Spec {
        name,
        abbr,
        value: false,
        negatable,
    }
}

pub(crate) const fn option(name: &'static str, abbr: Option<char>) -> Spec {
    Spec {
        name,
        abbr,
        value: true,
        negatable: false,
    }
}

impl Arguments {
    /// `package:args` parsing of `args` against `specs`.
    pub fn parse(args: &[String], specs: &[Spec], command: &str) -> Result<Self, Failure> {
        let mut result = Self {
            flags: BTreeMap::new(),
            options: BTreeMap::new(),
            rest: Vec::new(),
        };
        let unknown = |text: &str| {
            usage(format!(
                "Could not find an option named \"{text}\".\n\nRun \"dart pub {command} --help\" for more information."
            ))
        };
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            index += 1;
            if arg == "--" {
                result.rest.extend(args[index..].iter().cloned());
                break;
            }
            if let Some(long) = arg.strip_prefix("--") {
                let (name, inline) = match long.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_owned())),
                    None => (long, None),
                };
                if let Some(spec) = specs.iter().find(|s| s.name == name) {
                    if spec.value {
                        let value = match inline {
                            Some(value) => value,
                            None => {
                                let value = args.get(index).cloned().ok_or_else(|| {
                                    usage(format!("Missing argument for \"{name}\"."))
                                })?;
                                index += 1;
                                value
                            }
                        };
                        result
                            .options
                            .entry(name.to_owned())
                            .or_default()
                            .push(value);
                    } else {
                        if inline.is_some() {
                            return Err(usage(format!(
                                "Flag option \"{name}\" should not be given a value."
                            )));
                        }
                        result.flags.insert(name.to_owned(), true);
                    }
                    continue;
                }
                if let Some(positive) = name.strip_prefix("no-")
                    && let Some(spec) = specs.iter().find(|s| s.name == positive && !s.value)
                {
                    if !spec.negatable {
                        return Err(usage(format!(
                            "Cannot negate option \"{positive}\".\n\nRun \"dart pub {command} --help\" for more information."
                        )));
                    }
                    result.flags.insert(positive.to_owned(), false);
                    continue;
                }
                return Err(unknown(name));
            }
            if let Some(short) = arg.strip_prefix('-')
                && !short.is_empty()
            {
                for (offset, c) in short.char_indices() {
                    let spec = specs
                        .iter()
                        .find(|s| s.abbr == Some(c))
                        .ok_or_else(|| unknown(&c.to_string()))?;
                    if spec.value {
                        let rest = &short[offset + c.len_utf8()..];
                        let value = if rest.is_empty() {
                            let value = args.get(index).cloned().ok_or_else(|| {
                                usage(format!("Missing argument for \"{}\".", spec.name))
                            })?;
                            index += 1;
                            value
                        } else {
                            rest.to_owned()
                        };
                        result
                            .options
                            .entry(spec.name.to_owned())
                            .or_default()
                            .push(value);
                        break;
                    }
                    result.flags.insert(spec.name.to_owned(), true);
                }
                continue;
            }
            result.rest.push(arg.clone());
        }
        Ok(result)
    }

    pub fn flag(&self, name: &str) -> Option<bool> {
        self.flags.get(name).copied()
    }

    pub fn is(&self, name: &str) -> bool {
        self.flag(name).unwrap_or(false)
    }

    pub fn option(&self, name: &str) -> Option<&str> {
        self.options
            .get(name)
            .and_then(|v| v.last())
            .map(String::as_str)
    }
}

/// What every command needs to reach packages.
pub(crate) struct Settings {
    pub cache_root: PathBuf,
    pub host: String,
    pub sdk: Version,
    pub sdk_root: Option<PathBuf>,
}

impl Settings {
    pub fn discover(sdk_lib: Option<PathBuf>, sdk_version: Option<&str>) -> Result<Self, Failure> {
        let sdk_lib = sdk_lib.or_else(dartforge_elements::sdk::SdkLayout::discover);
        let sdk_root = sdk_lib
            .map(|lib| {
                let lib = lib
                    .canonicalize()
                    .map_err(|error| format!("{}: {error}", lib.display()))?;
                if !lib.join("libraries.json").is_file() {
                    return Err("--sdk must select the SDK lib directory".to_string());
                }
                lib.parent()
                    .map(PathBuf::from)
                    .ok_or_else(|| "SDK root is missing".to_string())
            })
            .transpose()?;
        let sdk = match sdk_version {
            Some(version) => Version::parse(version)?,
            None => match &sdk_root {
                Some(root) => sdk::version(root)?,
                None => Version::parse("3.6.2")?,
            },
        };
        let host = hosted::normalize_host(
            &std::env::var("PUB_HOSTED_URL").unwrap_or_else(|_| "https://pub.dev".into()),
        )
        .map_err(|e| data(format!("Invalid `PUB_HOSTED_URL`: {e}")))?;
        let cache = std::env::var_os("PUB_CACHE")
            .filter(|c| !c.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".pub-cache")))
            .ok_or("set PUB_CACHE to the package cache directory")?;
        let cache_root = if cache.is_absolute() {
            normalize(&cache)
        } else {
            normalize(
                &std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(cache),
            )
        };
        Ok(Self {
            cache_root,
            host,
            sdk,
            sdk_root,
        })
    }

    pub fn cache(&self, offline: bool) -> Cache {
        Cache::new(
            self.cache_root.clone(),
            offline,
            self.host.clone(),
            self.sdk.clone(),
            self.sdk_root.clone(),
        )
    }
}

/// The DartForge-specific options every subcommand accepts.
pub(crate) const COMMON: [Spec; 3] = [
    option("sdk", None),
    option("sdk-version", None),
    flag("help", Some('h'), false),
];

/// Runs `dartforge pub <args>` and returns the exit code.
pub(crate) fn run(args: &[OsString]) -> Result<u8, Failure> {
    let args: Vec<String> = args
        .iter()
        .map(|a| {
            a.to_str()
                .map(str::to_owned)
                .ok_or("pub arguments must be UTF-8")
        })
        .collect::<Result<_, _>>()?;
    // Pub's global flags come before the subcommand.
    let mut index = 0;
    while index < args.len() && args[index].starts_with('-') {
        match args[index].as_str() {
            "-v" | "--verbose" | "--trace" | "--no-trace" | "--color" | "--no-color" => {}
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(0);
            }
            other => {
                return Err(usage(format!(
                    "Could not find an option named \"{}\".\n\n{USAGE}",
                    other.trim_start_matches('-')
                )));
            }
        }
        index += 1;
    }
    let Some(command) = args.get(index) else {
        return Err(usage(USAGE));
    };
    let rest = &args[index + 1..];
    match command.as_str() {
        "get" => commands::get(rest),
        "upgrade" => commands::upgrade(rest),
        "downgrade" => commands::downgrade(rest),
        "add" => commands::add(rest),
        "remove" => commands::remove(rest),
        "deps" => commands::deps(rest),
        "outdated" => commands::outdated(rest),
        other => Err(usage(format!(
            "Could not find a subcommand named \"{other}\" for \"dart pub\".\n\n{USAGE}"
        ))),
    }
}

/// The workspace a command works in (Pub's `Entrypoint`).
#[derive(Clone)]
pub(crate) struct Entrypoint {
    /// Packages in `transitiveWorkspace` order; the root comes first.
    pub workspace: Vec<WorkspacePackage>,
    /// The package the command was run in.
    pub work: usize,
    /// Pub's `presentationDir` of the root.
    pub location: String,
    /// The root directory as Pub refers to it (`.` for the working one).
    pub root_dir: String,
    /// Whether the root's overrides come from `pubspec_overrides.yaml`.
    pub overrides_file: bool,
}

/// `p.isWithin(parent, child)` for absolute normalized paths.
fn is_within(parent: &Path, child: &Path) -> bool {
    child != parent && child.starts_with(parent)
}

/// Pub's `parentDirs`: the directory as given, then its parents, as Pub
/// spells them, each with its absolute path.
fn parent_dirs(path: &str, cwd: &Path) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    let mut absolute = normalize(&cwd.join(path));
    let mut shown = path.to_owned();
    let mut relative = false;
    loop {
        result.push((
            if relative {
                output::relative_path(&absolute, cwd)
            } else {
                shown.clone()
            },
            absolute.clone(),
        ));
        if !is_within(cwd, &absolute) {
            relative = true;
        }
        let Some(parent) = absolute.parent().map(Path::to_path_buf) else {
            break;
        };
        absolute = parent;
        if !relative {
            shown = match Path::new(&shown).parent() {
                Some(p) if !p.as_os_str().is_empty() => p.display().to_string(),
                _ => ".".into(),
            };
        }
    }
    result
}

impl Entrypoint {
    /// `Entrypoint._loadWorkspace`.
    pub fn load(directory: &str, settings: &Settings) -> Result<Self, Failure> {
        let cwd = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let mut met: Vec<(PathBuf, Manifest)> = Vec::new();
        for (shown, absolute) in parent_dirs(directory, &cwd) {
            if !absolute.join("pubspec.yaml").is_file() {
                continue;
            }
            let manifest = read_workspace_manifest(&absolute, settings)?;
            let resolution = manifest.resolution;
            met.push((absolute.clone(), manifest.clone()));
            if resolution != Resolution::None {
                continue;
            }
            let mut workspace = Vec::new();
            load_package(
                manifest,
                absolute.clone(),
                shown.clone(),
                settings,
                &met,
                &mut workspace,
            )?;
            if let Some(work) = workspace.iter().position(|p| p.directory == met[0].0) {
                validate_workspace(&workspace)?;
                // Pub's `presentationDir`.
                let location = if is_within(&absolute, &cwd) {
                    absolute.display().to_string()
                } else {
                    shown.clone()
                };
                let overrides_file = absolute.join("pubspec_overrides.yaml").is_file()
                    && std::fs::read_to_string(absolute.join("pubspec_overrides.yaml"))
                        .ok()
                        .and_then(|t| tree::parse_yaml(&t).ok())
                        .is_some_and(|n| n.contains("dependency_overrides"));
                return Ok(Self {
                    workspace,
                    work,
                    location,
                    root_dir: shown,
                    overrides_file,
                });
            }
        }
        match met.first() {
            None => Err(Failure {
                code: exit::NO_INPUT,
                message: format!(
                    "Found no `pubspec.yaml` file in `{}` or parent directories",
                    normalize(&cwd.join(directory)).display()
                ),
            }),
            Some((path, manifest)) => Err(Failure {
                code: exit::NO_INPUT,
                message: format!(
                    "Found a pubspec.yaml at {}. But it has resolution `{}`.\nBut found no workspace root including it in parent directories.\n\nSee https://dart.dev/go/pub-workspaces for more information.",
                    path.display(),
                    manifest.resolution.name()
                ),
            }),
        }
    }

    pub fn root(&self) -> &WorkspacePackage {
        &self.workspace[0]
    }

    /// `p.join(workspaceRoot.dir, 'example')`.
    pub fn example_dir(&self) -> String {
        format!("{}/example", self.root_dir.trim_end_matches('/'))
    }

    /// Pub's suffix for messages about this workspace.
    pub fn suffix(&self) -> String {
        if self.root_dir == "." {
            String::new()
        } else {
            format!(" in `{}`", self.location)
        }
    }
}

fn read_workspace_manifest(directory: &Path, settings: &Settings) -> Result<Manifest, Failure> {
    let description = Description::Root(directory.to_path_buf());
    let context = Context {
        containing: &description,
        host: &settings.host,
        sdk: &settings.sdk,
    };
    Manifest::read(directory, &context, Role::Workspace).map_err(data)
}

/// `Package.load`: a package and its workspace members, depth first.
fn load_package(
    manifest: Manifest,
    directory: PathBuf,
    shown: String,
    settings: &Settings,
    met: &[(PathBuf, Manifest)],
    packages: &mut Vec<WorkspacePackage>,
) -> Result<usize, Failure> {
    let index = packages.len();
    let members = manifest.workspace.clone();
    packages.push(WorkspacePackage {
        manifest,
        directory: directory.clone(),
        shown: shown.clone(),
        children: Vec::new(),
    });
    let mut children = Vec::new();
    for member in members {
        let path = normalize(&directory.join(&member));
        if !path.join("pubspec.yaml").is_file() {
            return Err(Failure {
                code: exit::NO_INPUT,
                message: format!(
                    "Could not find a file named \"pubspec.yaml\" in \"{}\".\nThat was included in the workspace of {}.",
                    path.display(),
                    directory.join("pubspec.yaml").display()
                ),
            });
        }
        let child = match met.iter().find(|(p, _)| *p == path) {
            Some((_, manifest)) => manifest.clone(),
            None => read_workspace_manifest(&path, settings)?,
        };
        if child.resolution != Resolution::Workspace {
            return Err(format!(
                "{} is included in the workspace from {}, but does not have `resolution: workspace`.\n\nSee https://dart.dev/go/pub-workspaces for more information.",
                path.join("pubspec.yaml").display(),
                directory.join("pubspec.yaml").display()
            )
            .into());
        }
        let child_shown = format!("{}/{member}", shown.trim_end_matches('/'));
        children.push(load_package(
            child,
            path,
            child_shown,
            settings,
            met,
            packages,
        )?);
    }
    packages[index].children = children;
    Ok(index)
}

/// Pub's `validateWorkspace`.
fn validate_workspace(workspace: &[WorkspacePackage]) -> Result<(), Failure> {
    if workspace[0].children.is_empty() {
        return Ok(());
    }
    let mut included_from: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    for package in workspace {
        for &child in &package.children {
            let child = &workspace[child];
            if let Some(previous) = included_from.get(&child.directory) {
                if *previous == package.directory {
                    return Err(format!(
                        "Packages can only be included in the workspace once.\n\n`{}` is included twice into the workspace of `{}`",
                        child.directory.join("pubspec.yaml").display(),
                        package.directory.join("pubspec.yaml").display()
                    )
                    .into());
                }
                return Err(format!(
                    "Packages can only be included in the workspace once.\n\n`{}` is included in the workspace, both from:\n* `{}` and\n* `{}`.",
                    child.directory.join("pubspec.yaml").display(),
                    package.directory.join("pubspec.yaml").display(),
                    previous.join("pubspec.yaml").display()
                )
                .into());
            }
            included_from.insert(child.directory.clone(), package.directory.clone());
        }
    }
    let mut names: BTreeMap<&str, &WorkspacePackage> = BTreeMap::new();
    for package in workspace {
        if let Some(collision) = names.insert(&package.manifest.name, package) {
            return Err(format!(
                "Workspace members must have unique names.\n`{}` and `{}` are both called \"{}\".",
                collision.directory.join("pubspec.yaml").display(),
                package.directory.join("pubspec.yaml").display(),
                package.manifest.name
            )
            .into());
        }
    }
    let mut overrides: BTreeMap<&str, &WorkspacePackage> = BTreeMap::new();
    for package in workspace {
        for dependency in &package.manifest.overrides {
            if let Some(collision) = overrides.insert(&dependency.name, package) {
                return Err(format!(
                    "The package `{}` is overridden in both:\npackage `{}` at `{}` and '{}' at `{}`.\n\nConsider removing one of the overrides.",
                    dependency.name,
                    collision.manifest.name,
                    collision.directory.display(),
                    package.manifest.name,
                    package.directory.display()
                )
                .into());
            }
            if let Some(member) = names.get(dependency.name.as_str()) {
                return Err(format!(
                    "Cannot override workspace packages.\n\nPackage `{}` at `{}` is overridden in `{}`.",
                    dependency.name,
                    member.directory.display(),
                    package.directory.join("pubspec.yaml").display()
                )
                .into());
            }
        }
    }
    let root = &workspace[0].directory;
    let mut visited: HashSet<PathBuf> = HashSet::from([root.clone()]);
    for package in workspace.iter().skip(1) {
        for directory in package.directory.ancestors().skip(1) {
            if included_from.contains_key(directory) || !visited.insert(directory.to_path_buf()) {
                break;
            }
            let candidate = directory.join("pubspec.yaml");
            if candidate.is_file() {
                return Err(format!(
                    "The file `{}` is located in a directory between the workspace root at\n`{}` and a workspace package at `{}`. But is not a member of the\nworkspace.\n\nThis blocks the resolution of the package at `{}`.\n\nConsider removing it.\n\nSee https://dart.dev/go/workspaces-stray-files for details.",
                    candidate.display(),
                    root.display(),
                    package.directory.display(),
                    package.directory.display()
                )
                .into());
            }
        }
    }
    Ok(())
}

/// Options of `acquireDependencies`.
#[derive(Default)]
pub(crate) struct Acquire {
    pub unlock: Vec<String>,
    pub dry_run: bool,
    pub summary_only: bool,
    pub enforce_lockfile: bool,
}

/// `Entrypoint._checkSdkConstraint` for the root pubspec.
fn check_root_sdk(entry: &Entrypoint, sdk: &Version) -> Result<(), Failure> {
    let root = entry.root();
    let pubspec = root.directory.join("pubspec.yaml");
    let effective = &root.manifest.dart_sdk().effective;
    let mut suggested = VersionSet::compatible_with(&Version::new(sdk.major, sdk.minor, 0));
    if !suggested.allows(sdk) {
        suggested = VersionSet::from_range(version::Range::user(
            Some(sdk.clone()),
            true,
            Some(sdk.next_breaking()),
            false,
        ))
        .as_compatible_with_if_possible();
    }
    let lower = match effective.ranges() {
        [range] => range.min.clone(),
        _ => None,
    };
    if lower.is_none() {
        return Err(data(format!(
            "{} has no lower-bound SDK constraint.\nYou should edit {} to contain an SDK constraint:\n\nenvironment:\n  sdk: '{suggested}'\n\nSee https://dart.dev/go/sdk-constraint\n",
            pubspec.display(),
            pubspec.display()
        )));
    }
    if manifest::language_of(effective) < (2, 12) {
        return Err(data(format!(
            "The lower bound of \"sdk: '{effective}'\" must be 2.12.0'\nor higher to enable null safety.\n\nThe current Dart SDK ({sdk}) only supports null safety.\n\nFor details, see https://dart.dev/null-safety\n"
        )));
    }
    for (name, _) in &root.manifest.sdks {
        if !source::SDKS.contains(&name.as_str()) {
            return Err(Failure {
                code: 1,
                message: format!(
                    "{} refers to an unknown sdk '{name}'.\n\nDid you mean to add it as a dependency?\n\nEither remove the constraint, or upgrade to a version of pub that supports the\ngiven sdk.\n\nSee https://dart.dev/go/sdk-constraint\n",
                    pubspec.display()
                ),
            });
        }
    }
    Ok(())
}

/// A solve with Pub's error reporting.
pub(crate) fn solve(
    entry: &Entrypoint,
    cache: &mut Cache,
    kind: SolveType,
    lock: &LockFile,
    unlock: &[String],
) -> Result<solver::SolveResult, Failure> {
    let cache_sdk = cache.sdk.clone();
    let solver = Solver::new(kind, cache, &entry.workspace, lock, unlock.iter().cloned());
    match solver.solve() {
        Ok(result) => Ok(result),
        Err(SolveError::Fatal(message)) => Err(message.into()),
        Err(SolveError::Failure {
            incompatibilities,
            root,
        }) => {
            let not_found = failure::package_not_found(&incompatibilities, root);
            let message = failure::explain(&incompatibilities, root, &cache_sdk);
            Err(Failure {
                code: if not_found { exit::UNAVAILABLE } else { 1 },
                message,
            })
        }
    }
}

/// The lockfile of a resolution, after downloading cached packages.
pub(crate) fn new_lock(
    entry: &Entrypoint,
    cache: &mut Cache,
    result: &solver::SolveResult,
    overridden: &BTreeSet<String>,
) -> Result<NewLock, Failure> {
    let mut packages = BTreeMap::new();
    for id in &result.packages {
        if id.is_root() {
            continue;
        }
        let id = cache.download(id).map_err(|e| Failure {
            code: exit::UNAVAILABLE,
            message: e,
        })?;
        packages.insert(id.name.clone(), id);
    }
    let mut sdks: BTreeMap<String, VersionSet> = BTreeMap::new();
    for id in &result.packages {
        if overridden.contains(&id.name) {
            continue;
        }
        let manifest = &result.pubspecs[&id.name];
        for (name, constraint) in &manifest.sdks {
            let current = sdks.remove(name).unwrap_or_else(VersionSet::any);
            sdks.insert(name.clone(), constraint.effective.intersect(&current));
        }
    }
    let root = &entry.root().manifest;
    Ok(NewLock {
        packages,
        sdks,
        main: root.dependencies.iter().map(|d| d.name.clone()).collect(),
        dev: root.dev.iter().map(|d| d.name.clone()).collect(),
        overridden: overridden.iter().cloned().collect(),
    })
}

pub(crate) fn overridden_names(entry: &Entrypoint) -> BTreeSet<String> {
    entry
        .workspace
        .iter()
        .flat_map(|p| p.manifest.overrides.iter().map(|d| d.name.clone()))
        .collect()
}

/// Pub's `acquireDependencies`.
pub(crate) fn acquire(
    entry: &Entrypoint,
    settings: &Settings,
    offline: bool,
    kind: SolveType,
    options: &Acquire,
) -> Result<(), Failure> {
    let root = entry.root();
    let suffix = entry.suffix();
    let lock_path = root.directory.join("pubspec.lock");
    if options.enforce_lockfile && !lock_path.is_file() {
        return Err(format!(
            "Retrieving dependencies failed{suffix}.\nCannot do `--enforce-lockfile` without an existing `pubspec.lock`.\n\nTry running `dart pub get` to create `{}`.",
            output::relative_path(&lock_path, &std::env::current_dir().unwrap_or_default())
        )
        .into());
    }
    let lock = LockFile::read(&root.directory, &settings.host).map_err(data)?;
    let mut cache = settings.cache(offline);
    println!("Resolving dependencies{suffix}...");
    check_root_sdk(entry, &settings.sdk)?;
    let result = solve(entry, &mut cache, kind, &lock, &options.unlock)?;
    println!("Downloading packages...");
    let overridden = overridden_names(entry);
    let new = new_lock(entry, &mut cache, &result, &overridden)?;
    let report = report::Report {
        kind,
        location: &entry.location,
        root: &root.manifest,
        overrides_file: entry.overrides_file,
        overridden: &overridden,
        previous: &lock,
        new: &new,
        available: &result.available,
        dry_run: options.dry_run,
        enforce_lockfile: options.enforce_lockfile,
        quiet: options.summary_only,
    };
    report.show(&cache);
    let same = lock.packages.len() == new.packages.len()
        && new
            .packages
            .iter()
            .all(|(name, id)| lock.packages.get(name) == Some(id));
    if options.enforce_lockfile && !same {
        let lock_display =
            output::relative_path(&lock_path, &std::env::current_dir().unwrap_or_default());
        return Err(data(format!(
            "Unable to satisfy `{}` using `{lock_display}`{suffix}.\n\nTo update `{lock_display}` run `dart pub get`{suffix} without\n`--enforce-lockfile`.",
            output::relative_path(
                &root.directory.join("pubspec.yaml"),
                &std::env::current_dir().unwrap_or_default()
            )
        )));
    }
    if !(options.dry_run || options.enforce_lockfile) {
        output::write_lock(&new, &root.directory)?;
    }
    if !options.dry_run {
        output::remove_stray_files(&entry.workspace)?;
        output::write_package_config(&entry.workspace, &new, &mut cache)?;
    }
    Ok(())
}

/// Shares the selected SDK mapping with distribution assembly.
#[cfg(feature = "native")]
pub(crate) fn sdk_package_paths(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, PathBuf>, String> {
    sdk::declared_package_paths(root)
}
