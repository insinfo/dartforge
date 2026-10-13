//! The builder executor's dependencies that the project does not resolve
//! (DF-BUILD-004): the host cannot rely on the project bringing
//! `build_runner` or `build_resolvers`. The Dart side of the executor
//! (`packages/build_executor`) imports [`DEPENDENCIES`]; when the project's
//! `package_config.json` does not resolve some of them, the host looks for
//! them in the pub cache (`PUB_CACHE`, or the platform default), with the
//! constraints of the executor's `pubspec.yaml`, and then for their
//! dependencies that are also missing, with the constraints of whoever asks
//! for them — always the highest cached version that satisfies them, as pub
//! would choose among the ones it already has. A package the project already
//! resolves stays at the project's version.
//!
//! There is no network: what the cache does not have is diagnosed by name, by
//! constraint and by who requested it.
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The packages the Dart side of the executor imports, with the constraints of
/// the `pubspec.yaml` of `packages/build_executor`.
pub const DEPENDENCIES: &[(&str, &str)] = &[
    ("analyzer", ">=6.9.0 <8.0.0"),
    ("build", "^2.4.2"),
    ("build_resolvers", "^2.4.4"),
    ("glob", "^2.1.0"),
    ("logging", "^1.2.0"),
    ("package_config", "^2.1.0"),
];

/// A semantic version `major.minor.patch[-pre][+build]` (the `+build` does not
/// count for ordering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub numbers: [u64; 3],
    pub pre: Option<String>,
}

impl Version {
    /// Parses `1.2.3`, `1.2.3-dev.1`, `1.2.3+4`.
    ///
    /// ```
    /// use dartforge_build::executor_deps::Version;
    /// assert!(Version::parse("2.4.2").unwrap() < Version::parse("2.10.0").unwrap());
    /// assert!(Version::parse("3.0.0-dev.1").unwrap() < Version::parse("3.0.0").unwrap());
    /// ```
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim();
        let without_build = text.split('+').next()?;
        let (core, pre) = match without_build.split_once('-') {
            Some((n, p)) => (n, Some(p.to_string())),
            None => (without_build, None),
        };
        let mut parts = core.split('.');
        let mut numbers = [0u64; 3];
        for n in &mut numbers {
            *n = parts.next()?.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Version { numbers, pre })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        self.numbers
            .cmp(&other.numbers)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

/// A pub version constraint: `any`, `1.2.3`, `^1.2.3`, and ranges with `>=`,
/// `>`, `<=`, `<` separated by spaces.
///
/// ```
/// use dartforge_build::executor_deps::{Version, satisfies};
/// let v = |t| Version::parse(t).unwrap();
/// assert!(satisfies("^2.4.2", &v("2.9.0")) && !satisfies("^2.4.2", &v("3.0.0")));
/// assert!(satisfies("^0.2.1", &v("0.2.9")) && !satisfies("^0.2.1", &v("0.3.0")));
/// assert!(satisfies(">=6.9.0 <8.0.0", &v("7.7.1")));
/// assert!(!satisfies("^1.0.0", &v("1.1.0-dev")));
/// ```
pub fn satisfies(constraint: &str, v: &Version) -> bool {
    let constraint = constraint.trim().trim_matches(|c| c == '\'' || c == '"');
    // Pre-releases only when the constraint itself mentions one.
    if v.pre.is_some() && !constraint.contains('-') {
        return false;
    }
    if constraint.is_empty() || constraint == "any" {
        return true;
    }
    if let Some(base) = constraint.strip_prefix('^') {
        let Some(b) = Version::parse(base) else {
            return false;
        };
        let ceiling = match b.numbers {
            [0, 0, p] => [0, 0, p + 1],
            [0, m, _] => [0, m + 1, 0],
            [x, _, _] => [x + 1, 0, 0],
        };
        return *v >= b && v.numbers < ceiling;
    }
    let mut rest = constraint;
    let mut any = false;
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        let (op, after) = [">=", "<=", ">", "<"]
            .iter()
            .find_map(|op| rest.strip_prefix(op).map(|d| (*op, d)))
            .unwrap_or(("=", rest));
        let after = after.trim_start();
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        let Some(b) = Version::parse(&after[..end]) else {
            return false;
        };
        let ok = match op {
            ">=" => *v >= b,
            ">" => *v > b,
            "<=" => *v <= b,
            "<" => *v < b,
            _ => *v == b,
        };
        if !ok {
            return false;
        }
        any = true;
        rest = &after[end..];
    }
    any
}

/// The `hosted/pub.dev` directory of the pub cache: `PUB_CACHE`, otherwise
/// `%LOCALAPPDATA%\Pub\Cache` (Windows) or `~/.pub-cache`.
pub fn pub_cache_dir() -> Option<PathBuf> {
    let root = std::env::var_os("PUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| {
            if cfg!(windows) {
                std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Pub").join("Cache"))
            } else {
                std::env::var_os("HOME").map(|d| PathBuf::from(d).join(".pub-cache"))
            }
        })?;
    Some(root.join("hosted").join("pub.dev"))
}

/// The highest version of `name` in the cache that satisfies `constraint`.
fn best_in_cache(cache: &Path, name: &str, constraint: &str) -> Option<(Version, PathBuf)> {
    let prefix = format!("{name}-");
    std::fs::read_dir(cache)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            let v = Version::parse(n.strip_prefix(&prefix)?)?;
            (satisfies(constraint, &v) && e.path().join("pubspec.yaml").is_file())
                .then(|| (v, e.path()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
}

/// The `dependencies` of a `pubspec.yaml`: name → constraint (those from a
/// source other than pub.dev get `any`).
fn pubspec_dependencies(text: &str) -> (Vec<(String, String)>, Option<String>) {
    use yaml_rust2::{Yaml, YamlLoader};
    let Ok(docs) = YamlLoader::load_from_str(text) else {
        return (Vec::new(), None);
    };
    let Some(doc) = docs.first() else {
        return (Vec::new(), None);
    };
    let mut deps = Vec::new();
    if let Yaml::Hash(h) = &doc["dependencies"] {
        for (k, v) in h {
            let Some(name) = k.as_str() else { continue };
            let r = match v {
                Yaml::String(s) => s.clone(),
                Yaml::Real(s) => s.clone(),
                Yaml::Hash(_) => v["version"].as_str().unwrap_or("any").to_string(),
                _ => "any".to_string(),
            };
            deps.push((name.to_string(), r));
        }
    }
    let sdk = doc["environment"]["sdk"].as_str().map(str::to_string);
    (deps, sdk)
}

/// A package's language version from its SDK constraint (the lowest accepted
/// version, `major.minor`), as pub writes it to `package_config.json`.
fn language_version(sdk: Option<&str>) -> String {
    let base = sdk
        .and_then(|s| {
            let s = s.trim();
            let s = s
                .strip_prefix('^')
                .or_else(|| s.strip_prefix(">="))
                .unwrap_or(s);
            Version::parse(s.split_whitespace().next()?)
        })
        .map_or([2, 12, 0], |v| v.numbers);
    format!("{}.{}", base[0], base[1])
}

/// The project's `package_config.json` (`original`) completed with the
/// executor packages it does not resolve, taken from `cache` (the
/// `hosted/pub.dev` of the pub cache). Returns the JSON and the added names.
///
/// # Errors
///
/// The `package_config` has no `packages`, or the cache has no version that
/// satisfies some missing package (the message says which one, with the
/// constraint and who requested it).
pub fn complete(original: &Value, cache: &Path) -> Result<(Value, Vec<String>), String> {
    let mut v = original.clone();
    let packages = v
        .get_mut("packages")
        .and_then(Value::as_array_mut)
        .ok_or("package_config without 'packages'")?;
    let present: Vec<String> = packages
        .iter()
        .filter_map(|p| p["name"].as_str().map(str::to_string))
        .collect();
    let mut pending: Vec<(String, String, String)> = DEPENDENCIES
        .iter()
        .map(|(n, r)| {
            (
                n.to_string(),
                r.to_string(),
                "dartforge_build_executor".to_string(),
            )
        })
        .collect();
    let mut chosen: BTreeMap<String, (Version, PathBuf, Option<String>)> = BTreeMap::new();
    let mut missing = Vec::new();
    while let Some((name, constraint, requester)) = pending.pop() {
        if present.contains(&name) || chosen.contains_key(&name) {
            continue;
        }
        let Some((version, dir)) = best_in_cache(cache, &name, &constraint) else {
            missing.push(format!("{name} {constraint} (requested by {requester})"));
            continue;
        };
        let text = std::fs::read_to_string(dir.join("pubspec.yaml")).unwrap_or_default();
        let (deps, sdk) = pubspec_dependencies(&text);
        for (d, r) in deps {
            pending.push((d, r, name.clone()));
        }
        chosen.insert(name, (version, dir, sdk));
    }
    if !missing.is_empty() {
        missing.sort();
        return Err(format!(
            "the builder executor needs packages that the project does not resolve and that the pub \
             cache ({}) does not have: {}. Declare them in the project's dev_dependencies and resolve \
             the dependencies",
            cache.display(),
            missing.join(", ")
        ));
    }
    let mut added = Vec::new();
    for (name, (_, dir, sdk)) in chosen {
        let uri = url::Url::from_directory_path(&dir)
            .map(|u| u.to_string())
            .unwrap_or_default();
        packages.push(json!({
            "name": name,
            "rootUri": uri,
            "packageUri": "lib/",
            "languageVersion": language_version(sdk.as_deref()),
        }));
        added.push(name);
    }
    Ok((v, added))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(cache: &Path, name: &str, version: &str, deps: &str) {
        let d = cache.join(format!("{name}-{version}"));
        std::fs::create_dir_all(d.join("lib")).unwrap();
        std::fs::write(
            d.join("pubspec.yaml"),
            format!("name: {name}\nversion: {version}\nenvironment:\n  sdk: '>=3.4.0 <4.0.0'\ndependencies:\n{deps}"),
        )
        .unwrap();
    }

    #[test]
    fn completes_from_cache_with_transitive_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();
        package(
            cache,
            "build_resolvers",
            "2.4.4",
            "  analyzer: '>=6.9.0 <8.0.0'\n  build: ^2.0.0\n",
        );
        package(cache, "build_resolvers", "3.0.0", "");
        package(cache, "analyzer", "6.11.0", "");
        package(cache, "analyzer", "7.7.1", "");
        package(cache, "analyzer", "8.0.0", "");
        package(cache, "logging", "1.3.0", "");
        package(cache, "package_config", "2.2.0", "");
        let original = json!({"configVersion": 2, "packages": [
            {"name": "build", "rootUri": "file:///b/", "packageUri": "lib/"},
            {"name": "glob", "rootUri": "file:///g/", "packageUri": "lib/"},
        ]});
        let (v, new) = complete(&original, cache).unwrap();
        assert_eq!(
            new,
            vec!["analyzer", "build_resolvers", "logging", "package_config"]
        );
        let ps = v["packages"].as_array().unwrap();
        let root = |n: &str| {
            ps.iter().find(|p| p["name"] == n).unwrap()["rootUri"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert!(root("build_resolvers").ends_with("build_resolvers-2.4.4/"));
        assert!(root("analyzer").ends_with("analyzer-7.7.1/"));
        // The project's `build` stays.
        assert_eq!(root("build"), "file:///b/");
        assert_eq!(
            ps.iter().find(|p| p["name"] == "logging").unwrap()["languageVersion"],
            "3.4"
        );
    }

    #[test]
    fn diagnoses_what_the_cache_lacks() {
        let dir = tempfile::tempdir().unwrap();
        let original = json!({"configVersion": 2, "packages": []});
        let e = complete(&original, dir.path()).unwrap_err();
        assert!(
            e.contains("build_resolvers ^2.4.4 (requested by dartforge_build_executor)"),
            "{e}"
        );
    }
}
