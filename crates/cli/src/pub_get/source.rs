//! Pub's system cache: the versions available for a package reference and
//! the pubspec of each version, for every source.
use super::{
    git::Git,
    hosted::{Hosted, Listing},
    manifest::{Context, Dependency, Description, Manifest, Role, normalize},
    tree::{Node, parse_yaml},
    version::{Version, VersionSet},
};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    rc::Rc,
};

/// A package at one version from one source (Pub's `PackageId`); the
/// resolved description is the archive hash of a hosted package or the
/// commit of a Git package.
#[derive(Clone, Debug)]
pub(crate) struct PackageId {
    pub name: String,
    pub version: Version,
    pub description: Description,
    pub resolved: Option<String>,
}

impl PartialEq for PackageId {
    /// An unknown hosted hash equals any hash, as in Pub.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.version == other.version
            && self.description == other.description
            && match (&self.description, &self.resolved, &other.resolved) {
                (Description::Hosted(_), Some(a), Some(b)) => a == b,
                (Description::Hosted(_), _, _) => true,
                (_, a, b) => a == b,
            }
    }
}

impl PackageId {
    pub fn to_range(&self) -> Dependency {
        Dependency {
            name: self.name.clone(),
            description: self.description.clone(),
            constraint: VersionSet::exact(&self.version),
        }
    }

    pub fn is_root(&self) -> bool {
        self.description.is_root()
    }
}

/// Why a source could not list or describe a package.
#[derive(Debug)]
pub(crate) enum SourceError {
    /// Pub's `PackageNotFoundException`: the solver turns it into a fact.
    NotFound(String),
    /// The pubspec of this version is invalid; the version is skipped.
    Invalid(String),
    /// Anything else stops the command.
    Fatal(String),
}

impl From<String> for SourceError {
    fn from(error: String) -> Self {
        Self::Fatal(error)
    }
}

/// The SDKs Pub knows, in its order.
pub(crate) const SDKS: [&str; 3] = ["dart", "flutter", "fuchsia"];

pub(crate) struct Cache {
    pub root: PathBuf,
    pub offline: bool,
    pub host: String,
    pub sdk: Version,
    /// The selected Dart SDK root, for `sdk: dart` packages.
    pub sdk_root: Option<PathBuf>,
    pub hosted: Hosted,
    pub git: Git,
    pubspecs: HashMap<(String, String, Description, Option<String>), Rc<Manifest>>,
}

impl Cache {
    pub fn new(
        root: PathBuf,
        offline: bool,
        host: String,
        sdk: Version,
        sdk_root: Option<PathBuf>,
    ) -> Self {
        Self {
            hosted: Hosted::new(root.clone(), offline),
            git: Git::new(root.clone(), offline),
            root,
            offline,
            host,
            sdk,
            sdk_root,
            pubspecs: HashMap::new(),
        }
    }

    /// Whether an SDK can provide packages and its version.
    pub fn sdk_version(&self, sdk: &str) -> Option<Version> {
        (sdk == "dart").then(|| self.sdk.clone())
    }

    fn context<'a>(&'a self, containing: &'a Description) -> Context<'a> {
        Context {
            containing,
            host: &self.host,
            sdk: &self.sdk,
        }
    }

    fn remember(&mut self, id: &PackageId, manifest: Manifest) -> Rc<Manifest> {
        let manifest = Rc::new(manifest);
        self.pubspecs.insert(
            (
                id.name.clone(),
                id.version.to_string(),
                id.description.clone(),
                id.resolved.clone(),
            ),
            manifest.clone(),
        );
        manifest
    }

    pub fn listing(&mut self, name: &str, url: &str) -> Result<Rc<Listing>, SourceError> {
        let host = self.host.clone();
        let sdk = self.sdk.clone();
        self.hosted.listing(name, url, &host, &sdk)
    }

    /// `SystemCache.getVersions`: every version of `name` from
    /// `description`, without retracted versions other than `retracted`.
    pub fn versions(
        &mut self,
        name: &str,
        description: &Description,
        retracted: Option<&Version>,
    ) -> Result<Vec<PackageId>, SourceError> {
        match description {
            Description::Hosted(url) => {
                if self.offline {
                    return self.offline_versions(name, url);
                }
                let listing = self.listing(name, url)?;
                Ok(listing
                    .versions
                    .iter()
                    .filter(|l| !l.retracted || Some(&l.version) == retracted)
                    .map(|l| PackageId {
                        name: name.to_owned(),
                        version: l.version.clone(),
                        description: description.clone(),
                        resolved: l.sha256.clone(),
                    })
                    .collect())
            }
            Description::Path { path, .. } => {
                let manifest = self.path_manifest(name, path, description)?;
                let id = PackageId {
                    name: name.to_owned(),
                    version: manifest.version.clone(),
                    description: description.clone(),
                    resolved: None,
                };
                self.remember(&id, manifest);
                Ok(vec![id])
            }
            Description::Git {
                url,
                reference,
                path,
                ..
            } => {
                let commit = self
                    .git
                    .resolve(url, reference)
                    .map_err(SourceError::Fatal)?;
                let manifest = self.git_manifest(name, description, url, path, &commit)?;
                let id = PackageId {
                    name: name.to_owned(),
                    version: manifest.version.clone(),
                    description: description.clone(),
                    resolved: Some(commit),
                };
                self.remember(&id, manifest);
                Ok(vec![id])
            }
            Description::Sdk(sdk) => {
                let (manifest, _) = self.sdk_manifest(name, sdk, description)?;
                let id = PackageId {
                    name: name.to_owned(),
                    version: manifest.version.clone(),
                    description: description.clone(),
                    resolved: None,
                };
                self.remember(&id, manifest);
                Ok(vec![id])
            }
            Description::Root(_) | Description::Unknown { .. } => Err(SourceError::Fatal(format!(
                "cannot list versions of {name}"
            ))),
        }
    }

    fn offline_versions(&mut self, name: &str, url: &str) -> Result<Vec<PackageId>, SourceError> {
        let directory = self
            .root
            .join("hosted")
            .join(super::hosted::url_to_directory(url));
        let mut result = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let Some((package, version)) = file_name.split_once('-') else {
                    continue;
                };
                if package != name || !entry.path().is_dir() {
                    continue;
                }
                let Ok(version) = Version::parse(version) else {
                    continue;
                };
                if version == Version::new(0, 0, 0) {
                    continue;
                }
                let resolved = self.hosted.cached_hash(name, &version, url);
                result.push(PackageId {
                    name: name.to_owned(),
                    version,
                    description: Description::Hosted(url.to_owned()),
                    resolved,
                });
            }
        }
        if result.is_empty() {
            return Err(SourceError::NotFound(format!(
                "could not find package {name} in cache"
            )));
        }
        Ok(result)
    }

    fn path_manifest(
        &self,
        name: &str,
        path: &Path,
        description: &Description,
    ) -> Result<Manifest, SourceError> {
        if path.is_dir() {
            if !path.join("pubspec.yaml").is_file() {
                return Err(SourceError::NotFound(format!(
                    "No pubspec.yaml found for package {name} in {}.",
                    path.display()
                )));
            }
        } else if path.is_file() {
            return Err(SourceError::NotFound(format!(
                "Path dependency for package {name} must refer to a directory, not a file. Was \"{}\".",
                path.display()
            )));
        } else {
            return Err(SourceError::NotFound(format!(
                "could not find package {name} at \"{}\"",
                description.format(&std::env::current_dir().unwrap_or_default())
            )));
        }
        let manifest = Manifest::read(path, &self.context(description), Role::Dependency)?;
        expect_name(
            &manifest,
            name,
            &path.join("pubspec.yaml").display().to_string(),
        )?;
        Ok(manifest)
    }

    fn git_manifest(
        &mut self,
        name: &str,
        description: &Description,
        url: &str,
        path: &str,
        commit: &str,
    ) -> Result<Manifest, SourceError> {
        let text = self
            .git
            .pubspec(url, path, commit)
            .map_err(SourceError::Fatal)?;
        let node = parse_yaml(&text).map_err(SourceError::Fatal)?;
        let manifest = Manifest::parse(&node, None, &self.context(description), Role::Dependency)?;
        expect_name(&manifest, name, &format!("{url} {commit}"))?;
        Ok(manifest)
    }

    fn sdk_manifest(
        &self,
        name: &str,
        sdk: &str,
        description: &Description,
    ) -> Result<(Manifest, PathBuf), SourceError> {
        if !SDKS.contains(&sdk) {
            return Err(SourceError::NotFound(format!("unknown SDK \"{sdk}\"")));
        }
        if sdk != "dart" {
            let title = if sdk == "flutter" {
                "Flutter"
            } else {
                "Fuchsia"
            };
            return Err(SourceError::NotFound(format!(
                "the {title} SDK is not available"
            )));
        }
        let directory = super::sdk::package_directory(self.sdk_root.as_deref(), name, &self.sdk)?;
        let manifest = Manifest::read(&directory, &self.context(description), Role::Dependency)?;
        expect_name(&manifest, name, &directory.display().to_string())?;
        if let Some(dependency) = manifest
            .dependencies
            .iter()
            .find(|d| !matches!(d.description, Description::Sdk(_)))
        {
            return Err(SourceError::Fatal(format!(
                "Only SDK packages are allowed as regular dependencies for packages vendored by the dart SDK, but the `{name}` package has a {} dependency on `{}`.",
                dependency.description.source(),
                dependency.name
            )));
        }
        Ok((manifest, directory))
    }

    /// `SystemCache.describe`: the pubspec of `id`.
    pub fn describe(&mut self, id: &PackageId) -> Result<Rc<Manifest>, SourceError> {
        let key = (
            id.name.clone(),
            id.version.to_string(),
            id.description.clone(),
            id.resolved.clone(),
        );
        if let Some(manifest) = self.pubspecs.get(&key) {
            return Ok(manifest.clone());
        }
        let manifest = match &id.description {
            Description::Hosted(url) => {
                // A downloaded package is described by its own pubspec.
                let directory = self.hosted.directory(&id.name, &id.version, url);
                if directory.join("pubspec.yaml").is_file() {
                    let manifest = Manifest::read(
                        &directory,
                        &self.context(&id.description),
                        Role::Dependency,
                    )
                    .map_err(SourceError::Invalid)?;
                    expect_name(&manifest, &id.name, &directory.display().to_string())?;
                    manifest
                } else if self.offline {
                    return Err(SourceError::NotFound(format!(
                        "{} {} is not available in cache",
                        id.name, id.version
                    )));
                } else {
                    let listing = self.listing(&id.name, url)?;
                    listing
                        .versions
                        .iter()
                        .find(|l| l.version == id.version)
                        .map(|l| l.manifest.clone())
                        .ok_or_else(|| {
                            SourceError::NotFound(format!(
                                "Could not find package {} {} at {url}",
                                id.name, id.version
                            ))
                        })?
                }
            }
            Description::Path { path, .. } => {
                self.path_manifest(&id.name, path, &id.description)?
            }
            Description::Git { url, path, .. } => {
                let commit = id.resolved.clone().ok_or_else(|| {
                    SourceError::Fatal(format!("{}: no resolved commit", id.name))
                })?;
                self.git
                    .ensure_commit(url, &commit)
                    .map_err(SourceError::Fatal)?;
                self.git_manifest(&id.name, &id.description.clone(), url, path, &commit)?
            }
            Description::Sdk(sdk) => self.sdk_manifest(&id.name, sdk, &id.description)?.0,
            Description::Root(_) | Description::Unknown { .. } => {
                return Err(SourceError::Fatal(format!("cannot describe {}", id.name)));
            }
        };
        Ok(self.remember(id, manifest))
    }

    /// The directory that holds the package `id`.
    pub fn directory(&self, id: &PackageId) -> Result<PathBuf, String> {
        match &id.description {
            Description::Root(path) | Description::Path { path, .. } => Ok(path.clone()),
            Description::Hosted(url) => Ok(self.hosted.directory(&id.name, &id.version, url)),
            Description::Git { url, path, .. } => {
                let commit = id
                    .resolved
                    .as_deref()
                    .ok_or("missing resolved git commit")?;
                let checkout = self.git.checkout(url, commit);
                Ok(if path == "." {
                    checkout
                } else {
                    checkout.join(path)
                })
            }
            Description::Sdk(_) => {
                super::sdk::package_directory(self.sdk_root.as_deref(), &id.name, &self.sdk)
                    .map_err(|e| match e {
                        SourceError::NotFound(m)
                        | SourceError::Invalid(m)
                        | SourceError::Fatal(m) => m,
                    })
            }
            Description::Unknown { .. } => Err(format!("{}: unknown source", id.name)),
        }
    }

    /// Downloads a cached package (hosted or Git) and returns its id with
    /// the content hash filled in.
    pub fn download(&mut self, id: &PackageId) -> Result<PackageId, String> {
        match &id.description {
            Description::Hosted(url) => {
                let listing = if self.offline {
                    None
                } else {
                    let listing = self.listing(&id.name, url).map_err(|e| match e {
                        SourceError::NotFound(m)
                        | SourceError::Invalid(m)
                        | SourceError::Fatal(m) => m,
                    })?;
                    listing
                        .versions
                        .iter()
                        .find(|l| l.version == id.version)
                        .cloned()
                };
                let sha256 = self
                    .hosted
                    .download(&id.name, &id.version, url, listing.as_ref())
                    .map_err(|e| format!("{} {}: {e}", id.name, id.version))?;
                Ok(PackageId {
                    resolved: Some(sha256),
                    ..id.clone()
                })
            }
            Description::Git { url, path, .. } => {
                let commit = id
                    .resolved
                    .as_deref()
                    .ok_or("missing resolved git commit")?;
                self.git.materialize(url, path, commit)?;
                Ok(id.clone())
            }
            _ => Ok(id.clone()),
        }
    }
}

fn expect_name(manifest: &Manifest, name: &str, location: &str) -> Result<(), SourceError> {
    if manifest.name != name {
        return Err(SourceError::Fatal(format!(
            "\"name\" field doesn't match expected name \"{name}\" in {location}."
        )));
    }
    Ok(())
}

/// The previous resolution recorded in `pubspec.lock`.
#[derive(Clone, Debug, Default)]
pub(crate) struct LockFile {
    pub packages: BTreeMap<String, PackageId>,
    /// `dependency:` of each package, as written.
    pub kinds: BTreeMap<String, String>,
}

impl LockFile {
    pub fn read(directory: &Path, host: &str) -> Result<Self, String> {
        let path = directory.join("pubspec.lock");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let node = parse_yaml(&text).map_err(|e| format!("Error on {}: {e}", path.display()))?;
        let mut lock = Self::default();
        let fail = |message: String| format!("Error on {}: {message}", path.display());
        for (name, spec) in node.get("packages").as_map().unwrap_or_default() {
            let version = Version::parse(
                spec.get("version")
                    .as_str()
                    .ok_or_else(|| fail(format!("{name}: missing version")))?,
            )
            .map_err(&fail)?;
            let description = spec.get("description");
            let text = |key: &str| -> Result<String, String> {
                description
                    .get(key)
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        fail(format!(
                            "{name}: the '{key}' field of the description must be a string."
                        ))
                    })
            };
            let (description, resolved) = match spec.get("source").as_str() {
                Some("hosted") => match description {
                    Node::String(package) if package == name => {
                        (Description::Hosted(host.to_owned()), None)
                    }
                    _ => (
                        Description::Hosted(super::hosted::normalize_host(&text("url")?)?),
                        description.get("sha256").as_str().map(str::to_owned),
                    ),
                },
                Some("path") => {
                    let relative = description.get("relative").as_bool().unwrap_or(false);
                    let written = text("path")?;
                    let path = if relative {
                        normalize(&directory.join(&written))
                    } else {
                        PathBuf::from(written)
                    };
                    (Description::Path { path, relative }, None)
                }
                Some("git") => {
                    let url = text("url")?;
                    let root = Description::Root(directory.to_path_buf());
                    let (url, relative) = super::manifest::resolve_git_url(&url, &root)?;
                    (
                        Description::Git {
                            url,
                            relative,
                            reference: description.get("ref").as_str().unwrap_or("HEAD").to_owned(),
                            path: text("path").unwrap_or_else(|_| ".".into()),
                        },
                        Some(text("resolved-ref")?),
                    )
                }
                Some("sdk") => (
                    Description::Sdk(
                        description
                            .as_str()
                            .ok_or_else(|| {
                                fail(format!("{name}: The description must be an SDK name."))
                            })?
                            .to_owned(),
                    ),
                    None,
                ),
                Some(other) => (
                    Description::Unknown {
                        source: other.to_owned(),
                        description: description.to_json(),
                    },
                    None,
                ),
                None => return Err(fail(format!("{name}: missing source"))),
            };
            if let Some(kind) = spec.get("dependency").as_str() {
                lock.kinds.insert(name.clone(), kind.to_owned());
            }
            lock.packages.insert(
                name.clone(),
                PackageId {
                    name: name.clone(),
                    version,
                    description,
                    resolved,
                },
            );
        }
        Ok(lock)
    }
}
