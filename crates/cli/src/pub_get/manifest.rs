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
}
#[derive(Clone, Debug)]
pub(super) struct Dependency {
    pub source: Source,
    pub constraint: Constraint,
}
#[derive(Clone, Debug)]
pub(super) struct Manifest {
    pub name: String,
    pub version: Version,
    pub sdk: Constraint,
    pub dependencies: BTreeMap<String, Dependency>,
    pub dev: BTreeMap<String, Dependency>,
    pub overrides: BTreeMap<String, Dependency>,
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
            value
                .as_object_mut()
                .ok_or("pubspec must be a mapping")?
                .remove("dev_dependencies");
            value
                .as_object_mut()
                .unwrap()
                .remove("dependency_overrides");
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
        for key in ["workspace", "resolution"] {
            if value.get(key).is_some() && directory.is_some() {
                return Err(format!("pub get does not yet support {key}"));
            }
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
        Ok(Self {
            name,
            version,
            sdk,
            dependencies,
            dev,
            overrides,
        })
    }
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
            }
        } else {
            let map = value
                .as_object()
                .ok_or("dependency must be a constraint or mapping")?;
            if map.contains_key("git") {
                return Err(format!("{name}: git package sources are not yet supported"));
            }
            if let Some(sdk) = value.get("sdk") {
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
                let path = directory
                    .join(path)
                    .canonicalize()
                    .map_err(|e| format!("{name}: invalid path {path}: {e}"))?;
                Dependency {
                    source: Source::Path(path),
                    constraint: Constraint::parse("any")?,
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
