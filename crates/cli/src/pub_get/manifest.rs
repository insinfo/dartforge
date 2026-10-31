//! Pubspecs as Pub 3.6.2 reads them: package references with their source
//! descriptions, SDK constraints with Pub's reinterpretations, and the
//! workspace fields.
//!
//! The dependencies of packages other than the workspace packages are
//! parsed but their errors are kept, because Pub only reports an invalid
//! pubspec of a dependency when the solver actually needs it.
use super::{
    tree::{Node, parse_yaml},
    version::{Range, Version, VersionSet},
};
use std::{
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
};

/// Where a package comes from, as far as its identity is concerned
/// (Pub's `Description`).
#[derive(Clone, Debug)]
pub(crate) enum Description {
    /// A workspace package, by its directory.
    Root(PathBuf),
    /// A package on a package server, by its normalized URL.
    Hosted(String),
    /// A local directory; `relative` only affects how it is written.
    Path { path: PathBuf, relative: bool },
    /// A Git repository, the reference to resolve and the package path
    /// inside it; `relative` marks a local repository written relative to
    /// the pubspec, which `url` holds resolved as a `file:` URL.
    Git {
        url: String,
        relative: bool,
        reference: String,
        path: String,
    },
    /// A package vendored by an SDK (`dart`, `flutter`, `fuchsia` or an
    /// unknown name).
    Sdk(String),
    /// A source Pub does not know, by its name and JSON description.
    Unknown { source: String, description: String },
}

impl PartialEq for Description {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Root(a), Self::Root(b)) => a == b,
            (Self::Hosted(a), Self::Hosted(b)) => a == b,
            (Self::Path { path: a, .. }, Self::Path { path: b, .. }) => a == b,
            (
                Self::Git {
                    url: a,
                    reference: ra,
                    path: pa,
                    ..
                },
                Self::Git {
                    url: b,
                    reference: rb,
                    path: pb,
                    ..
                },
            ) => a == b && ra == rb && pa == pb,
            (Self::Sdk(a), Self::Sdk(b)) => a == b,
            (
                Self::Unknown {
                    source: a,
                    description: da,
                },
                Self::Unknown {
                    source: b,
                    description: db,
                },
            ) => a == b && da == db,
            _ => false,
        }
    }
}
impl Eq for Description {}

impl Hash for Description {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Root(path) | Self::Path { path, .. } => path.hash(state),
            Self::Hosted(url) => url.hash(state),
            Self::Git {
                url,
                reference,
                path,
                ..
            } => (url, reference, path).hash(state),
            Self::Sdk(name) => name.hash(state),
            Self::Unknown {
                source,
                description,
            } => (source, description).hash(state),
        }
    }
}

impl Description {
    /// Pub's source name, as the lockfile writes it.
    pub fn source(&self) -> &str {
        match self {
            Self::Root(_) => "root",
            Self::Hosted(_) => "hosted",
            Self::Path { .. } => "path",
            Self::Git { .. } => "git",
            Self::Sdk(_) => "sdk",
            Self::Unknown { source, .. } => source,
        }
    }

    pub fn is_root(&self) -> bool {
        matches!(self, Self::Root(_))
    }

    /// Only hosted packages have more than one version to choose from.
    pub fn has_multiple_versions(&self) -> bool {
        matches!(self, Self::Hosted(_))
    }

    /// Pub's `Description.format`, used in messages.
    pub fn format(&self, from: &Path) -> String {
        match self {
            Self::Root(path) => super::output::relative_path(path, from),
            Self::Hosted(url) => format!("on {url}"),
            Self::Path { path, relative } => {
                if *relative {
                    super::output::relative_path(path, from)
                } else {
                    path.display().to_string()
                }
            }
            Self::Git {
                url,
                reference,
                path,
                ..
            } => {
                let mut text = format!("{url} at {reference}");
                if path != "." {
                    text.push_str(&format!(" in {path}"));
                }
                text
            }
            Self::Sdk(sdk) => sdk.clone(),
            Self::Unknown { description, .. } => description.clone(),
        }
    }
}

/// A dependency: a package name, its source and the allowed versions
/// (Pub's `PackageRange`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Dependency {
    pub name: String,
    pub description: Description,
    pub constraint: VersionSet,
}

/// An SDK constraint as written and as Pub interprets it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SdkConstraint {
    pub original: VersionSet,
    pub effective: VersionSet,
}

/// `resolution:` of a pubspec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    None,
    Workspace,
    Local,
    External,
}

impl Resolution {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Workspace => "workspace",
            Self::Local => "local",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Manifest {
    pub name: String,
    pub version: Version,
    /// The `dart` constraint first, then the other `environment` entries in
    /// written order.
    pub sdks: Vec<(String, SdkConstraint)>,
    pub dependencies: Vec<Dependency>,
    pub dev: Vec<Dependency>,
    pub overrides: Vec<Dependency>,
    /// `workspace:` entries as written.
    pub workspace: Vec<String>,
    pub resolution: Resolution,
    /// A deferred error of a dependency's pubspec (its `dependencies` or
    /// `environment`), reported when the solver reads them.
    pub error: Option<String>,
}

pub(crate) fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && (bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

/// `p.normalize` of an absolute path: `.` and `..` are resolved lexically.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push("..");
                }
            }
            other => result.push(other.as_os_str()),
        }
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}

/// What a pubspec is read for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// A workspace package: every section is parsed and errors are fatal.
    Workspace,
    /// A dependency: only `dependencies` and `environment` matter, and
    /// their errors are deferred.
    Dependency,
}

pub(crate) struct Context<'a> {
    /// The description of the package that contains the pubspec; relative
    /// paths and Git paths resolve against it.
    pub containing: &'a Description,
    /// The default package server.
    pub host: &'a str,
    /// The running Dart SDK version.
    pub sdk: &'a Version,
}

fn version_constraint(node: &Node, what: &str) -> Result<VersionSet, String> {
    match node {
        Node::Null => Ok(VersionSet::any()),
        Node::String(text) => {
            VersionSet::parse(text).map_err(|e| format!("Invalid version constraint: {e}"))
        }
        _ => Err(format!("A version constraint must be a string{what}.")),
    }
}

/// `SdkConstraint.interpretDartSdkConstraint` with the default upper bound
/// `<2.0.0` for constraints without one.
fn interpret_dart(original: &VersionSet, sdk: &Version) -> VersionSet {
    let mut constraint = original.clone();
    let default = VersionSet::parse("<2.0.0").expect("valid constraint");
    if let [range] = constraint.ranges()
        && range.max.is_none()
        && default.allows_any(&constraint)
    {
        constraint = constraint.intersect(&default);
    }
    if sdk.major >= 3
        && let [range] = constraint.ranges()
        && language_of(&constraint) >= (2, 12)
        && !range.include_max
        && range.max.as_ref() == Some(&Version::new(3, 0, 0).first_prerelease())
    {
        constraint = VersionSet::from_range(Range::user(
            range.min.clone(),
            range.include_min,
            Some(Version::new(4, 0, 0)),
            false,
        ));
    }
    constraint
}

/// `LanguageVersion.fromSdkConstraint`.
pub(crate) fn language_of(constraint: &VersionSet) -> (u64, u64) {
    constraint
        .ranges()
        .first()
        .and_then(|r| r.min.as_ref())
        .map(|v| (v.major, v.minor))
        .unwrap_or((2, 7))
}

fn parse_environment(node: &Node, sdk: &Version) -> Result<Vec<(String, SdkConstraint)>, String> {
    let original = match node {
        Node::Null => VersionSet::any(),
        Node::Map(_) => version_constraint(node.get("sdk"), "")?,
        _ => return Err("\"environment\" field must be a map.".into()),
    };
    let mut result = vec![(
        "dart".to_owned(),
        SdkConstraint {
            effective: interpret_dart(&original, sdk),
            original,
        },
    )];
    for (name, value) in node.as_map().unwrap_or_default() {
        if name == "dart" {
            return Err("Use \"sdk\" to for Dart SDK constraints.".into());
        }
        if name == "sdk" {
            continue;
        }
        let constraint = version_constraint(value, "")?;
        let effective = match (name.as_str(), constraint.ranges()) {
            // Flutter does not use semantic versions for breaking releases,
            // so Pub ignores its upper bounds.
            ("flutter", [range]) => VersionSet::from_range(Range {
                min: range.min.clone(),
                include_min: range.include_min,
                max: None,
                include_max: false,
            }),
            _ => constraint.clone(),
        };
        result.push((
            name.clone(),
            SdkConstraint {
                original: constraint,
                effective,
            },
        ));
    }
    Ok(result)
}

/// `GitSource._validatedPath`.
fn git_path(path: &Node) -> Result<String, String> {
    let path = match path {
        Node::Null => ".",
        Node::String(path) => path.as_str(),
        _ => return Err("The 'path' field of the description must be a string.".into()),
    };
    if path.starts_with('/') || path.contains("://") || path.contains(['?', '#']) {
        return Err("The 'path' field of the description must be a relative path URL.".into());
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("The 'path' field of the description must not reach outside the repository.".into());
                }
            }
            part => parts.push(part),
        }
    }
    Ok(if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    })
}

/// The directory a containing description resolves relative paths in.
fn containing_dir(containing: &Description) -> Option<&Path> {
    match containing {
        Description::Root(path) | Description::Path { path, .. } => Some(path),
        _ => None,
    }
}

/// `GitSource._validatedUrl`: a relative local repository is resolved
/// against the containing directory into a `file:` URL.
pub(crate) fn resolve_git_url(
    url: &str,
    containing: &Description,
) -> Result<(String, bool), String> {
    if url.contains('@') {
        return Ok((url.to_owned(), false));
    }
    let has_absolute_path = match url.split_once("://") {
        Some((_, rest)) => rest.contains('/'),
        None => match url.split_once(':') {
            Some((scheme, rest))
                if !scheme.is_empty()
                    && scheme
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b)) =>
            {
                rest.starts_with('/')
            }
            _ => url.starts_with('/'),
        },
    };
    if has_absolute_path {
        return Ok((url.to_owned(), false));
    }
    let Some(directory) = containing_dir(containing) else {
        return Err(format!(
            "\"{url}\" is a relative path, but this isn't a local pubspec."
        ));
    };
    let base = url::Url::from_directory_path(directory)
        .map_err(|_| format!("invalid directory {}", directory.display()))?;
    let joined = base
        .join(url)
        .map_err(|e| format!("invalid git url {url}: {e}"))?;
    Ok((joined.to_string().trim_end_matches('/').to_owned(), true))
}

fn parse_reference(
    name: &str,
    source: Option<&str>,
    description: &Node,
    context: &Context<'_>,
    language: (u64, u64),
) -> Result<Description, String> {
    match source {
        None | Some("hosted") => {
            let url = match description {
                Node::Null => context.host.to_owned(),
                Node::String(text) => {
                    if language >= (2, 15) {
                        text.clone()
                    } else if valid_name(text) && text == name {
                        context.host.to_owned()
                    } else {
                        return Err("Using `hosted: <url>` is only supported with a minimum SDK constraint of 2.15.".into());
                    }
                }
                Node::Map(_) => {
                    let package = match description.get("name") {
                        Node::Null if language >= (2, 15) => name,
                        Node::String(package) => package.as_str(),
                        _ => {
                            return Err("The 'name' key must have a string value without a minimum Dart SDK constraint of 2.15.0 or higher.".into());
                        }
                    };
                    if package != name {
                        return Err(format!(
                            "hosted package names that differ from the dependency name ({package}) are not supported"
                        ));
                    }
                    match description.get("url") {
                        Node::Null => context.host.to_owned(),
                        Node::String(url) => url.clone(),
                        _ => return Err("The 'url' key must be a string value.".into()),
                    }
                }
                _ => return Err("The description must be a package name or map.".into()),
            };
            Ok(Description::Hosted(super::hosted::normalize_host(&url)?))
        }
        Some("path") => {
            let Node::String(path) = description else {
                return Err("The description must be a path string.".into());
            };
            let relative = Path::new(path).is_relative();
            match context.containing {
                Description::Root(directory) => Ok(Description::Path {
                    path: normalize(&directory.join(path)),
                    relative,
                }),
                Description::Path {
                    path: directory, ..
                } => Ok(Description::Path {
                    path: if relative {
                        normalize(&directory.join(path))
                    } else {
                        PathBuf::from(path)
                    },
                    relative,
                }),
                Description::Git {
                    url,
                    relative: url_relative,
                    reference,
                    path: base,
                } => {
                    if !relative {
                        return Err(format!(
                            "\"{path}\" is an absolute path, it can't be referenced from a git pubspec."
                        ));
                    }
                    let joined = if base == "." {
                        path.clone()
                    } else {
                        format!("{base}/{path}")
                    };
                    let resolved = git_path(&Node::String(joined)).map_err(|_| {
                        format!(
                            "the path \"{path}\" cannot refer outside the git repository {base}."
                        )
                    })?;
                    Ok(Description::Git {
                        url: url.clone(),
                        relative: *url_relative,
                        reference: reference.clone(),
                        path: resolved,
                    })
                }
                Description::Hosted(_) => {
                    if relative {
                        return Err(format!(
                            "\"{path}\" is a relative path, but this isn't a local pubspec."
                        ));
                    }
                    Ok(Description::Path {
                        path: PathBuf::from(path),
                        relative: false,
                    })
                }
                _ => Err(format!(
                    "\"{path}\" is a path, but this isn't a local pubspec."
                )),
            }
        }
        Some("git") => {
            let (url, reference, path) = match description {
                Node::String(url) => (url.as_str(), None, &Node::Null),
                Node::Map(_) => {
                    let Node::String(url) = description.get("url") else {
                        return Err("The 'url' field of a description must be a string.".into());
                    };
                    let reference = match description.get("ref") {
                        Node::Null => None,
                        Node::String(r) => Some(r.as_str()),
                        _ => {
                            return Err(
                                "The 'ref' field of the description must be a string.".into()
                            );
                        }
                    };
                    (url.as_str(), reference, description.get("path"))
                }
                _ => {
                    return Err(
                        "The description must be a Git URL or a map with a 'url' key.".into(),
                    );
                }
            };
            let (url, relative) = resolve_git_url(url, context.containing)?;
            Ok(Description::Git {
                url,
                relative,
                reference: reference.unwrap_or("HEAD").to_owned(),
                path: git_path(path)?,
            })
        }
        Some("sdk") => match description {
            Node::String(sdk) => Ok(Description::Sdk(sdk.clone())),
            _ => Err("The description must be an SDK name.".into()),
        },
        Some(other) => Ok(Description::Unknown {
            source: other.to_owned(),
            description: description.to_json(),
        }),
    }
}

fn parse_dependencies(
    field: &str,
    node: &Node,
    package: &str,
    context: &Context<'_>,
    language: (u64, u64),
) -> Result<Vec<Dependency>, String> {
    let entries = match node {
        Node::Null => return Ok(Vec::new()),
        Node::Map(entries) => entries,
        _ => return Err(format!("\"{field}\" field must be a map.")),
    };
    let mut result = Vec::with_capacity(entries.len());
    for (name, spec) in entries {
        if !valid_name(name) {
            return Err(format!("Not a valid package name: {name}."));
        }
        if name == package {
            return Err(format!(
                "A package may not list itself as a dependency ({name})."
            ));
        }
        let invalid = |e: String| {
            format!(
                "Invalid description in the \"{package}\" pubspec on the \"{name}\" dependency: {e}"
            )
        };
        let (source, description, constraint) = match spec {
            Node::Null => (None, &Node::Null, VersionSet::any()),
            Node::String(_) => (None, &Node::Null, version_constraint(spec, "")?),
            Node::Map(fields) => {
                let constraint = version_constraint(spec.get("version"), "")?;
                let others: Vec<_> = fields.iter().filter(|(k, _)| k != "version").collect();
                match others.as_slice() {
                    [] => (Some("hosted"), &Node::Null, constraint),
                    [(source, description)] => (Some(source.as_str()), description, constraint),
                    _ => return Err(format!("A dependency may only have one source ({name}).")),
                }
            }
            _ => {
                return Err(format!(
                    "A dependency specification must be a string or a mapping ({name})."
                ));
            }
        };
        let description =
            parse_reference(name, source, description, context, language).map_err(invalid)?;
        result.push(Dependency {
            name: name.clone(),
            description,
            constraint,
        });
    }
    Ok(result)
}

impl Manifest {
    /// The effective Dart SDK constraint.
    pub fn dart_sdk(&self) -> &SdkConstraint {
        &self.sdks[0].1
    }

    /// The language version implied by the Dart SDK constraint as written.
    pub fn language_version(&self) -> (u64, u64) {
        language_of(&self.dart_sdk().original)
    }

    pub fn dependency(&self, name: &str) -> Option<&Dependency> {
        self.dependencies.iter().find(|d| d.name == name)
    }

    pub fn dev_dependency(&self, name: &str) -> Option<&Dependency> {
        self.dev.iter().find(|d| d.name == name)
    }

    pub fn override_of(&self, name: &str) -> Option<&Dependency> {
        self.overrides.iter().find(|d| d.name == name)
    }

    /// A placeholder for a version whose pubspec could not be read (Pub's
    /// `Pubspec(name, version: version)`).
    pub fn empty(name: &str, version: Version) -> Self {
        Self {
            name: name.to_owned(),
            version,
            sdks: vec![(
                "dart".into(),
                SdkConstraint {
                    original: VersionSet::any(),
                    effective: VersionSet::any(),
                },
            )],
            dependencies: Vec::new(),
            dev: Vec::new(),
            overrides: Vec::new(),
            workspace: Vec::new(),
            resolution: Resolution::None,
            error: None,
        }
    }

    /// Reads a parsed pubspec. For [`Role::Workspace`], the overrides file
    /// `overrides` replaces `dependency_overrides`, `workspace` and
    /// `resolution` when given.
    pub fn parse(
        value: &Node,
        overrides: Option<&Node>,
        context: &Context<'_>,
        role: Role,
    ) -> Result<Self, String> {
        if !matches!(value, Node::Map(_)) {
            return Err("The pubspec must be a YAML mapping.".into());
        }
        let name = match value.get("name") {
            Node::String(name) => name.clone(),
            Node::Null => return Err("Missing the required \"name\" field.".into()),
            _ => return Err("\"name\" field must be a string.".into()),
        };
        let version = match value.get("version") {
            Node::Null => Version::new(0, 0, 0),
            Node::String(text) => Version::parse(text)
                .map_err(|e| format!("Invalid version in the \"{name}\" pubspec: {e}"))?,
            _ => {
                return Err(format!(
                    "\"version\" field must be a string in the \"{name}\" pubspec."
                ));
            }
        };
        let mut manifest = Self::empty(&name, version);
        let lazy = (|| -> Result<_, String> {
            let sdks = parse_environment(value.get("environment"), context.sdk)?;
            let language = language_of(&sdks[0].1.original);
            let dependencies = parse_dependencies(
                "dependencies",
                value.get("dependencies"),
                &name,
                context,
                language,
            )?;
            Ok((sdks, dependencies, language))
        })();
        let language = match lazy {
            Ok((sdks, dependencies, language)) => {
                manifest.sdks = sdks;
                manifest.dependencies = dependencies;
                language
            }
            Err(error) if role == Role::Dependency => {
                manifest.error = Some(error);
                return Ok(manifest);
            }
            Err(error) => return Err(error),
        };
        if role == Role::Dependency {
            return Ok(manifest);
        }
        manifest.dev = parse_dependencies(
            "dev_dependencies",
            value.get("dev_dependencies"),
            &name,
            context,
            language,
        )?;
        let pick = |key: &str| -> &Node {
            match overrides {
                Some(file) if file.contains(key) => file.get(key),
                _ => value.get(key),
            }
        };
        manifest.overrides = parse_dependencies(
            "dependency_overrides",
            pick("dependency_overrides"),
            &name,
            context,
            language,
        )?;
        for dependency in &manifest.dev {
            if manifest.dependency(&dependency.name).is_some() {
                return Err(format!(
                    "{} is a dependency and a dev dependency of {name}; a package may only be listed once.",
                    dependency.name
                ));
            }
        }
        let workspace = pick("workspace");
        let resolution = pick("resolution");
        let has = |key: &str| value.contains(key) || overrides.is_some_and(|o| o.contains(key));
        if (has("workspace") || has("resolution")) && language < (3, 5) {
            return Err(format!(
                "`workspace` and `resolution` requires at least language version 3.5\n\nConsider updating the SDK constraint to:\n\nenvironment:\n  sdk: '^{}'",
                context.sdk
            ));
        }
        match workspace {
            Node::Null => {}
            Node::List(items) => {
                for item in items {
                    let Node::String(path) = item else {
                        return Err("\"workspace\" must be a list of strings".into());
                    };
                    if Path::new(path).is_absolute() {
                        return Err("\"workspace\" members must be relative paths".into());
                    }
                    // `p.isWithin('.', path)`, lexically.
                    let mut depth = 0i64;
                    let mut escapes = false;
                    for part in path.split('/') {
                        match part {
                            "" | "." => {}
                            ".." => {
                                depth -= 1;
                                escapes |= depth < 0;
                            }
                            _ => depth += 1,
                        }
                    }
                    if escapes || depth == 0 {
                        return Err("\"workspace\" members must be subdirectories".into());
                    }
                    manifest.workspace.push(path.clone());
                }
            }
            _ => return Err("\"workspace\" must be a list of strings".into()),
        }
        manifest.resolution = match resolution {
            Node::Null => Resolution::None,
            Node::String(kind) if kind == "workspace" => Resolution::Workspace,
            Node::String(kind) if kind == "local" => Resolution::Local,
            Node::String(kind) if kind == "external" => Resolution::External,
            _ => {
                return Err(
                    "\"resolution\" must be one of `workspace`, `local`, `external`".into(),
                );
            }
        };
        Ok(manifest)
    }

    /// Reads `pubspec.yaml` (and `pubspec_overrides.yaml` for a workspace
    /// package) in `directory`.
    pub fn read(directory: &Path, context: &Context<'_>, role: Role) -> Result<Self, String> {
        let path = directory.join("pubspec.yaml");
        let text = std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "Could not find a file named \"pubspec.yaml\" in \"{}\".",
                    directory.display()
                )
            } else {
                format!("{}: {e}", path.display())
            }
        })?;
        let value = parse_yaml(&text).map_err(|e| format!("Error on {}: {e}", path.display()))?;
        let overrides = if role == Role::Workspace {
            let overrides_path = directory.join("pubspec_overrides.yaml");
            match std::fs::read_to_string(&overrides_path) {
                Ok(text) => {
                    let node = parse_yaml(&text)
                        .map_err(|e| format!("Error on {}: {e}", overrides_path.display()))?;
                    match &node {
                        Node::Null => None,
                        Node::Map(entries) => {
                            if let Some((key, _)) = entries.iter().find(|(k, _)| {
                                !matches!(
                                    k.as_str(),
                                    "dependency_overrides" | "resolution" | "workspace"
                                )
                            }) {
                                return Err(format!(
                                    "{}: pubspec_overrides.yaml only supports the `dependency_overrides`, `resolution` and `workspace` fields (found `{key}`).",
                                    overrides_path.display()
                                ));
                            }
                            Some(node)
                        }
                        _ => {
                            return Err(format!(
                                "{}: the overrides file must be a map",
                                overrides_path.display()
                            ));
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(format!("{}: {error}", overrides_path.display())),
            }
        } else {
            None
        };
        Self::parse(&value, overrides.as_ref(), context, role)
            .map_err(|e| format!("Error on {}: {e}", path.display()))
    }
}
