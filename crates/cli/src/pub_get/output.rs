//! Pub-compatible lockfile and package configuration generation.
use super::{
    hosted::atomic_write,
    manifest::{Manifest, Source, yaml_json},
    resolver::Selected,
};
use semver::Version;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub(super) fn read_lock(
    directory: &Path,
    host: &str,
) -> Result<BTreeMap<String, (Source, Version)>, String> {
    let path = directory.join("pubspec.lock");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let value = yaml_json(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    let mut result = BTreeMap::new();
    let packages = value["packages"]
        .as_object()
        .ok_or("pubspec.lock packages must be a mapping")?;
    for (name, p) in packages {
        let version = Version::parse(p["version"].as_str().ok_or("lockfile version is missing")?)
            .map_err(|e| e.to_string())?;
        let source = match p["source"].as_str() {
            Some("path") => {
                let path = p["description"]["path"]
                    .as_str()
                    .ok_or("lockfile path is missing")?;
                // A locked path that no longer exists (for instance one written
                // on another system) only stops matching, as in Pub.
                let joined = directory.join(path);
                Source::Path(joined.canonicalize().unwrap_or(joined))
            }
            Some("hosted") => Source::Hosted(super::hosted::normalize_host(
                p["description"]["url"].as_str().unwrap_or(host),
            )?),
            Some("git") => {
                let d = &p["description"];
                let text = |key: &str| {
                    d[key]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("lockfile git {key} is missing for {name}"))
                };
                Source::Git {
                    url: text("url")?,
                    reference: text("ref")?,
                    path: text("path")?,
                }
            }
            Some("sdk") => {
                let sdk = p["description"]
                    .as_str()
                    .ok_or("SDK lockfile description must be a name")?;
                if sdk != "dart" {
                    return Err(format!("unsupported locked SDK {sdk:?}"));
                }
                Source::Sdk(sdk.into())
            }
            _ => return Err(format!("unsupported locked source for {name}")),
        };
        result.insert(name.clone(), (source, version));
    }
    Ok(result)
}
/// The content identities the lockfile records: hosted archive hashes and
/// resolved Git commits.
pub(super) fn read_lock_digests(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = directory.join("pubspec.lock");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let value = yaml_json(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    Ok(value["packages"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, p)| {
            let description = &p["description"];
            description["sha256"]
                .as_str()
                .or(description["resolved-ref"].as_str())
                .map(|digest| (name.clone(), digest.to_owned()))
        })
        .collect())
}
/// Pub's `yamlToString`: identifier-like strings stay plain, everything
/// else is written as a JSON string, so lockfiles match Pub's text.
fn quote(text: &str) -> String {
    let mut bytes = text.bytes();
    let plain = bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b == b'-')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if plain {
        text.to_owned()
    } else {
        serde_json::to_string(text).expect("string serialization")
    }
}

/// Writes the resolution of a package that is not in a larger workspace.
#[cfg(test)]
pub(super) fn write(
    directory: &Path,
    root: &Manifest,
    selected: &BTreeMap<String, Selected>,
    sdk: &Version,
) -> Result<(), String> {
    write_workspace(&[(root.clone(), directory.to_path_buf())], selected, sdk)
}

/// `path` relative to `base`, both canonical, with `/` separators (Pub's
/// `p.relative` on POSIX).
pub(super) fn relative_path(path: &Path, base: &Path) -> String {
    let path: Vec<_> = path.components().collect();
    let base: Vec<_> = base.components().collect();
    let shared = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut parts = vec!["..".to_string(); base.len() - shared];
    parts.extend(
        path[shared..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// `p.toUri` of a relative path: segments are percent-encoded and a final
/// `..` segment gains a slash, as Dart's URI normalization does.
fn relative_uri(path: &str) -> String {
    let mut uri = String::new();
    for byte in path.bytes() {
        // RFC 3986 path characters stay as they are, like Dart's `Uri`.
        if byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    if path == ".." || path.ends_with("/..") {
        uri.push('/');
    }
    uri
}

fn file_uri(directory: &Path) -> Result<String, String> {
    url::Url::from_file_path(directory)
        .map(|u| u.to_string())
        .map_err(|_| format!("invalid package directory {}", directory.display()))
}

/// Writes `pubspec.lock` and `.dart_tool/package_config.json` at the
/// workspace root as Pub 3.6.2 lays them out, plus the `workspace_ref.json`
/// file of every workspace package, and removes the members' stray
/// resolution files like Pub does.
pub(super) fn write_workspace(
    workspace: &[(Manifest, PathBuf)],
    selected: &BTreeMap<String, Selected>,
    sdk: &Version,
) -> Result<(), String> {
    let (root, directory) = workspace.first().ok_or("empty workspace")?;
    let tool = directory.join(".dart_tool");
    let overridden: BTreeSet<&str> = workspace
        .iter()
        .flat_map(|(m, _)| m.overrides.keys().map(String::as_str))
        .collect();
    let mut lock = String::from("# Generated by dartforge pub get.\n");
    lock.push_str(if selected.is_empty() {
        "packages: {}\n"
    } else {
        "packages:\n"
    });
    let mut packages = Vec::new();
    for (name, value) in selected {
        // Pub's precedence: a main or development dependency of the root stays
        // direct even when it is overridden too.
        let kind = if root.dependencies.contains_key(name) {
            "direct main"
        } else if root.dev.contains_key(name) {
            "direct dev"
        } else if overridden.contains(name.as_str()) {
            "direct overridden"
        } else {
            "transitive"
        };
        lock.push_str(&format!("  {name}:\n    dependency: {}\n", quote(kind)));
        let root_uri = match &value.source {
            Source::Path(path) => {
                let (description, uri) = if value.relative {
                    (
                        relative_path(path, directory),
                        relative_uri(&relative_path(path, &tool)),
                    )
                } else {
                    (path.to_string_lossy().into_owned(), file_uri(path)?)
                };
                lock.push_str(&format!(
                    "    description:\n      path: {}\n      relative: {}\n    source: path\n",
                    quote(&description),
                    value.relative
                ));
                uri
            }
            Source::Hosted(host) => {
                lock.push_str(&format!(
                    "    description:\n      name: {}\n      sha256: {}\n      url: {}\n    source: hosted\n",
                    quote(name),
                    quote(
                        value
                            .candidate
                            .digest
                            .as_deref()
                            .ok_or("missing archive hash")?
                    ),
                    quote(host)
                ));
                file_uri(&value.candidate.directory)?
            }
            Source::Sdk(sdk) => {
                lock.push_str(&format!(
                    "    description: {}\n    source: sdk\n",
                    quote(sdk)
                ));
                file_uri(&value.candidate.directory)?
            }
            Source::Git {
                url,
                reference,
                path,
            } => {
                let commit = value
                    .candidate
                    .digest
                    .as_deref()
                    .ok_or("missing resolved git commit")?;
                lock.push_str(&format!(
                    "    description:\n      path: {}\n      ref: {}\n      resolved-ref: {}\n      url: {}\n    source: git\n",
                    quote(path),
                    quote(reference),
                    quote(commit),
                    quote(url)
                ));
                // `p.join(checkout, '.')` becomes a URI with a final slash.
                let uri = file_uri(&value.candidate.directory)?;
                if path == "." { format!("{uri}/") } else { uri }
            }
        };
        lock.push_str(&format!(
            "    version: {}\n",
            quote(&value.candidate.manifest.version.to_string())
        ));
        packages.push(config_package(
            name,
            &root_uri,
            &value.candidate.manifest,
            sdk,
        ));
    }
    let mut lower = None;
    let mut upper = None;
    for manifest in workspace.iter().map(|(m, _)| m).chain(
        selected
            .values()
            .filter(|v| !overridden.contains(v.candidate.manifest.name.as_str()))
            .map(|v| &v.candidate.manifest),
    ) {
        let constraint = manifest.sdk.for_sdk(sdk);
        if let Some(bound) = constraint.lower
            && lower.as_ref().is_none_or(|(v, inc): &(Version, bool)| {
                super::constraints::compare_versions(&bound.0, v).is_gt()
                    || (super::constraints::compare_versions(&bound.0, v).is_eq()
                        && !bound.1
                        && *inc)
            })
        {
            lower = Some(bound);
        }
        if let Some(bound) = constraint.upper
            && upper.as_ref().is_none_or(|(v, inc): &(Version, bool)| {
                super::constraints::compare_versions(&bound.0, v).is_lt()
                    || (super::constraints::compare_versions(&bound.0, v).is_eq()
                        && !bound.1
                        && *inc)
            })
        {
            upper = Some(bound);
        }
    }
    let mut sdk_parts = Vec::new();
    if let Some((v, inc)) = lower {
        sdk_parts.push(format!("{}{v}", if inc { ">=" } else { ">" }));
    }
    if let Some((v, inc)) = upper {
        sdk_parts.push(format!("{}{v}", if inc { "<=" } else { "<" }));
    }
    lock.push_str(&format!("sdks:\n  dart: {}\n", quote(&sdk_parts.join(" "))));
    // Resolved packages sorted by name, then the workspace in its own order.
    packages.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    for (manifest, package) in workspace {
        packages.push(config_package(
            &manifest.name,
            &relative_uri(&relative_path(package, &tool)),
            manifest,
            sdk,
        ));
    }
    // Pub's key order, which serde_json's sorted maps would not keep.
    let mut config = String::from("{\n  \"configVersion\": 2,\n  \"packages\": [\n");
    for (index, package) in packages.iter().enumerate() {
        config.push_str("    {\n");
        for (position, key) in ["name", "rootUri", "packageUri", "languageVersion"]
            .iter()
            .enumerate()
        {
            config.push_str(&format!(
                "      \"{key}\": {}{}\n",
                package[key],
                if position < 3 { "," } else { "" }
            ));
        }
        config.push_str(if index + 1 < packages.len() {
            "    },\n"
        } else {
            "    }\n"
        });
    }
    config.push_str(&format!(
        "  ],\n  \"generator\": \"dartforge\",\n  \"generatorVersion\": {}\n}}\n",
        serde_json::Value::from(env!("CARGO_PKG_VERSION"))
    ));
    // Like Pub's `writeTextFileIfDifferent`, an equivalent lockfile (for
    // instance one written by `dart pub get`) is left untouched.
    let lock_path = directory.join("pubspec.lock");
    let unchanged = std::fs::read_to_string(&lock_path)
        .ok()
        .and_then(|text| yaml_json(&text).ok())
        .is_some_and(|existing| yaml_json(&lock).is_ok_and(|new| new == existing));
    if !unchanged {
        atomic_write(&lock_path, lock.as_bytes())?;
    }
    atomic_write(&tool.join("package_config.json"), config.as_bytes())?;
    if root.workspace.is_empty() {
        return Ok(());
    }
    for (_, package) in workspace {
        let reference = json!({
            "workspaceRoot": relative_path(directory, &package.join(".dart_tool/pub"))
        });
        let mut bytes = serde_json::to_vec_pretty(&reference).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        atomic_write(&package.join(".dart_tool/pub/workspace_ref.json"), &bytes)?;
        if package == directory {
            continue;
        }
        for (stray, what) in [
            ("pubspec.lock", "lock-file"),
            (".dart_tool/package_config.json", "package config"),
        ] {
            let path = package.join(stray);
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                eprintln!("Deleting old {what}: `{}`.", path.display());
            }
        }
    }
    Ok(())
}

fn config_package(
    name: &str,
    root_uri: &str,
    manifest: &Manifest,
    sdk: &Version,
) -> serde_json::Value {
    json!({"name":name,"rootUri":root_uri,"packageUri":"lib/","languageVersion":manifest.sdk.language(sdk)})
}
