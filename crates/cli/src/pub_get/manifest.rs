//! Validated pubspec input shared by local packages and hosted metadata.
use super::constraints::{Constraint, parse_version};
use semver::Version;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use yaml_rust2::{Yaml, YamlLoader};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Path(PathBuf),
    Hosted(String),
    Sdk(String),
    /// A Git repository, the reference to resolve (`HEAD` by default) and
    /// the normalized package path inside it (`.` by default).
    Git {
        url: String,
        reference: String,
        path: String,
    },
}
#[derive(Clone, Debug)]
pub(super) struct Dependency {
    pub source: Source,
    pub constraint: Constraint,
    /// A path dependency written as a relative path; Pub keeps it relative
    /// in the lockfile and the package configuration.
    pub relative: bool,
}
#[derive(Clone, Debug)]
pub(super) struct Manifest {
    pub name: String,
    pub version: Version,
    pub sdk: Constraint,
    pub dependencies: BTreeMap<String, Dependency>,
    pub dev: BTreeMap<String, Dependency>,
    pub overrides: BTreeMap<String, Dependency>,
    /// Canonical directories of the `workspace` members, in written order.
    pub workspace: Vec<PathBuf>,
    /// `resolution: workspace` marks a workspace member.
    pub member: bool,
}
pub(super) fn yaml_json(text: &str) -> Result<Value, String> {
    fn convert(value: &Yaml) -> Result<Value, String> {
        Ok(match value {
            Yaml::Null => Value::Null,
            Yaml::String(s) => json!(s),
            Yaml::Integer(i) => json!(i),
            Yaml::Boolean(b) => json!(b),
            Yaml::Array(a) => Value::Array(a.iter().map(convert).collect::<Result<_, _>>()?),
            Yaml::Hash(h) => {
                let mut result = serde_json::Map::new();
                for (key, value) in h {
                    let key = key.as_str().ok_or("YAML keys must be strings")?;
                    if result.insert(key.into(), convert(value)?).is_some() {
                        return Err(format!("duplicate YAML key: {key}"));
                    }
                }
                Value::Object(result)
            }
            _ => return Err("unsupported YAML value or alias".into()),
        })
    }
    let docs = YamlLoader::load_from_str(text).map_err(|e| format!("invalid pubspec YAML: {e}"))?;
    if docs.len() != 1 {
        return Err("expected exactly one YAML document".into());
    }
    convert(&docs[0])
}
pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && (name.as_bytes()[0].is_ascii_lowercase() || name.as_bytes()[0] == b'_')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
impl Manifest {
    pub fn read(directory: &Path, host: &str, include_dev: bool) -> Result<Self, String> {
        let path = directory.join("pubspec.yaml");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut value = yaml_json(&text)?;
        if include_dev {
            let overrides_path = directory.join("pubspec_overrides.yaml");
            match std::fs::read_to_string(&overrides_path) {
                Ok(text) => {
                    let overrides = yaml_json(&text)?;
                    let overrides = overrides
                        .as_object()
                        .ok_or("pubspec overrides must be a mapping")?;
                    let root = value.as_object_mut().ok_or("pubspec must be a mapping")?;
                    // Pub replaces each supported top-level attribute, not individual entries.
                    for key in ["dependency_overrides", "workspace", "resolution"] {
                        if let Some(value) = overrides.get(key) {
                            root.insert(key.into(), value.clone());
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("{}: {error}", overrides_path.display())),
            }
        }
        if !include_dev {
            // Only workspace packages contribute these sections; a path
            // dependency's workspace layout does not belong to this resolution.
            let object = value.as_object_mut().ok_or("pubspec must be a mapping")?;
            for key in [
                "dev_dependencies",
                "dependency_overrides",
                "workspace",
                "resolution",
            ] {
                object.remove(key);
            }
        }
        Self::parse(&value, Some(directory), host)
    }
    pub fn read_hosted(directory: &Path, host: &str) -> Result<Self, String> {
        let path = directory.join("pubspec.yaml");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&yaml_json(&text)?, None, host)
    }
    pub fn parse(value: &Value, directory: Option<&Path>, host: &str) -> Result<Self, String> {
        if !value.is_object() {
            return Err("pubspec must be a mapping".into());
        }
        let name = value["name"]
            .as_str()
            .ok_or("pubspec requires a package name")?
            .to_string();
        if !valid_name(&name) {
            return Err(format!("invalid package name: {name}"));
        }
        let version = parse_version(value["version"].as_str().unwrap_or("0.0.0"))?;
        let sdk = match value.get("environment") {
            None => Constraint::parse("any")?,
            Some(env) => {
                let object = env.as_object().ok_or("environment must be a mapping")?;
                if object.keys().any(|k| k != "sdk") {
                    return Err(format!("{name}: unsupported non-Dart SDK constraint"));
                }
                Constraint::parse(
                    env["sdk"]
                        .as_str()
                        .ok_or("environment.sdk must be a version constraint")?,
                )?
            }
        };
        let dependencies = parse_dependencies(&value["dependencies"], directory, host)?;
        // Hosted dev dependencies are irrelevant and may legitimately use unsupported sources.
        let dev = if directory.is_some() {
            parse_dependencies(&value["dev_dependencies"], directory, host)?
        } else {
            BTreeMap::new()
        };
        let overrides = if directory.is_some() {
            parse_dependencies(&value["dependency_overrides"], directory, host)?
        } else {
            BTreeMap::new()
        };
        let (workspace, member) = match directory {
            Some(directory) => parse_workspace(value, directory, &name, &sdk)?,
            None => (Vec::new(), false),
        };
        Ok(Self {
            name,
            version,
            sdk,
            dependencies,
            dev,
            overrides,
            workspace,
            member,
        })
    }
}
/// Reads `workspace` and `resolution` as Pub 3.6 does for local packages.
fn parse_workspace(
    value: &Value,
    directory: &Path,
    name: &str,
    sdk: &Constraint,
) -> Result<(Vec<PathBuf>, bool), String> {
    let member = match value.get("resolution") {
        None | Some(Value::Null) => false,
        Some(Value::String(kind)) if kind == "workspace" => true,
        Some(Value::String(kind)) => {
            return Err(format!(
                "{name}: pub get does not yet support resolution: {kind}"
            ));
        }
        Some(_) => return Err(format!("{name}: resolution must be a string")),
    };
    let mut members = Vec::new();
    match value.get("workspace") {
        None | Some(Value::Null) => {}
        Some(Value::Array(entries)) => {
            for entry in entries {
                let path = entry
                    .as_str()
                    .ok_or_else(|| format!("{name}: workspace members must be paths"))?;
                if path.contains(['*', '?', '[', '{']) {
                    return Err(format!(
                        "{name}: workspace glob patterns are not supported: {path}"
                    ));
                }
                if Path::new(path).is_absolute() {
                    return Err(format!("{name}: workspace member {path} must be relative"));
                }
                let member = directory
                    .join(path)
                    .canonicalize()
                    .map_err(|e| format!("{name}: workspace member {path}: {e}"))?;
                members.push(member);
            }
        }
        Some(_) => return Err(format!("{name}: workspace must be a list of paths")),
    }
    if (member || value.get("workspace").is_some_and(|v| !v.is_null()))
        && sdk
            .lower
            .as_ref()
            .is_none_or(|(v, _)| (v.major, v.minor) < (3, 5))
    {
        return Err(format!(
            "{}: `workspace` and `resolution` requires at least language version 3.5",
            directory.join("pubspec.yaml").display()
        ));
    }
    Ok((members, member))
}
/// A Git description as Pub 3.6 reads it: a URL, or a map with `url`, an
/// optional `ref` and an optional relative `path`.
fn git_source(name: &str, description: &Value) -> Result<Source, String> {
    let (url, reference, path) = match description {
        Value::String(url) => (url.as_str(), None, None),
        Value::Object(fields) => {
            if let Some(key) = fields
                .keys()
                .find(|key| !matches!(key.as_str(), "url" | "ref" | "path"))
            {
                return Err(format!(
                    "{name}: git description field {key:?} is not supported"
                ));
            }
            let text = |key: &str| -> Result<Option<&str>, String> {
                fields
                    .get(key)
                    .filter(|v| !v.is_null())
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| format!("{name}: git {key} must be a string"))
                    })
                    .transpose()
            };
            (
                text("url")?.ok_or_else(|| format!("{name}: git description requires a url"))?,
                text("ref")?,
                text("path")?,
            )
        }
        _ => return Err(format!("{name}: git must be a URL or a description map")),
    };
    // Network URLs and SCP-like addresses are used as written; Pub resolves
    // relative local repositories against the pubspec, which DartForge does
    // not reproduce yet.
    let remote = url.contains("://")
        || url
            .split_once(':')
            .is_some_and(|(host, _)| host.contains('@') && !host.contains('/'));
    if !remote && !Path::new(url).is_absolute() {
        return Err(format!(
            "{name}: relative git repository paths are not supported yet: {url}"
        ));
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.unwrap_or(".").split('/') {
        match part {
            "" | "." => {}
            ".." if parts.pop().is_some() => {}
            ".." => return Err(format!("{name}: git path must stay inside the repository")),
            part => parts.push(part),
        }
    }
    if path.is_some_and(|p| p.starts_with('/')) {
        return Err(format!("{name}: git path must be relative"));
    }
    Ok(Source::Git {
        url: url.to_owned(),
        reference: reference.unwrap_or("HEAD").to_owned(),
        path: if parts.is_empty() {
            ".".into()
        } else {
            parts.join("/")
        },
    })
}
fn parse_dependencies(
    value: &Value,
    directory: Option<&Path>,
    host: &str,
) -> Result<BTreeMap<String, Dependency>, String> {
    if value.is_null() {
        return Ok(BTreeMap::new());
    }
    let mut result = BTreeMap::new();
    for (name, value) in value.as_object().ok_or("dependencies must be a mapping")? {
        if !valid_name(name) {
            return Err(format!("invalid dependency name: {name}"));
        }
        let dependency = if value.is_null() || value.is_string() {
            Dependency {
                source: Source::Hosted(host.into()),
                constraint: Constraint::parse(value.as_str().unwrap_or("any"))?,
                relative: false,
            }
        } else {
            let map = value
                .as_object()
                .ok_or("dependency must be a constraint or mapping")?;
            if let Some(git) = value.get("git") {
                if map.keys().any(|key| key != "git" && key != "version") {
                    return Err(format!("{name}: unsupported git dependency fields"));
                }
                Dependency {
                    source: git_source(name, git)?,
                    constraint: Constraint::parse(
                        value
                            .get("version")
                            .map(|v| v.as_str().ok_or("version must be a string"))
                            .transpose()?
                            .unwrap_or("any"),
                    )?,
                    relative: false,
                }
            } else if let Some(sdk) = value.get("sdk") {
                if map.keys().any(|key| key != "sdk" && key != "version") {
                    return Err(format!("{name}: unsupported SDK dependency fields"));
                }
                let sdk = sdk.as_str().ok_or("dependency sdk must be a name")?;
                if sdk != "dart" {
                    return Err(format!(
                        "{name}: SDK package source {sdk:?} is not supported"
                    ));
                }
                Dependency {
                    relative: false,
                    source: Source::Sdk(sdk.into()),
                    constraint: Constraint::parse(
                        value
                            .get("version")
                            .map(|value| value.as_str().ok_or("version must be a string"))
                            .transpose()?
                            .unwrap_or("any"),
                    )?,
                }
            } else if let Some(path) = value.get("path") {
                if map.len() != 1 {
                    return Err(format!("{name}: unsupported path dependency fields"));
                }
                let path = path.as_str().ok_or("dependency path must be a string")?;
                let directory =
                    directory.ok_or("hosted packages cannot declare local path dependencies")?;
                let relative = Path::new(path).is_relative();
                let path = directory
                    .join(path)
                    .canonicalize()
                    .map_err(|e| format!("{name}: invalid path {path}: {e}"))?;
                Dependency {
                    source: Source::Path(path),
                    constraint: Constraint::parse("any")?,
                    relative,
                }
            } else {
                if map.keys().any(|k| k != "hosted" && k != "version") {
                    return Err(format!("{name}: unsupported dependency source"));
                }
                let url = match value.get("hosted") {
                    None => host,
                    Some(Value::String(url)) => url,
                    Some(Value::Object(h)) => {
                        if h.keys().any(|k| k != "name" && k != "url")
                            || h.get("name")
                                .and_then(Value::as_str)
                                .is_some_and(|n| n != name)
                        {
                            return Err(format!("{name}: invalid hosted description"));
                        }
                        h.get("url")
                            .and_then(Value::as_str)
                            .ok_or("hosted.url must be a string")?
                    }
                    _ => return Err("hosted must be a URL or mapping".into()),
                };
                Dependency {
                    relative: false,
                    source: Source::Hosted(super::hosted::normalize_host(url)?),
                    constraint: Constraint::parse(
                        value
                            .get("version")
                            .map(|v| v.as_str().ok_or("version must be a string"))
                            .transpose()?
                            .unwrap_or("any"),
                    )?,
                }
            }
        };
        result.insert(name.clone(), dependency);
    }
    Ok(result)
}
