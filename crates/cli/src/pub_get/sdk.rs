//! Resolve only packages declared by the selected SDK's vendor configuration.
use super::{
    constraints::parse_version,
    hosted::Candidate,
    manifest::{Manifest, Source, valid_name, yaml_json},
};
use semver::Version;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(super) fn version(root: &Path) -> Result<Version, String> {
    let path = root.join("version");
    parse_version(
        std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .trim(),
    )
}

pub(super) fn candidate(
    root: Option<&Path>,
    sdk_name: &str,
    package: &str,
    expected_version: &Version,
    host: &str,
) -> Result<Candidate, String> {
    if sdk_name != "dart" {
        return Err(format!("unsupported package SDK {sdk_name:?}"));
    }
    let root =
        root.ok_or("Dart SDK packages require a selected SDK (use --sdk or DARTFORGE_SDK_LIB)")?;
    if version(root)? != *expected_version {
        return Err(format!(
            "selected Dart SDK version differs from resolution SDK {expected_version}"
        ));
    }
    let packages = declared_package_paths(root)?;
    let directory = packages
        .get(package)
        .map(|path| root.join(path))
        .ok_or_else(|| format!("package {package} is not declared by the selected Dart SDK"))?
        .canonicalize()
        .map_err(|error| format!("SDK package {package}: {error}"))?;
    let canonical_root = root.canonicalize().map_err(|error| format!("{}: {error}", root.display()))?;
    if !directory.starts_with(&canonical_root) {
        return Err(format!("SDK package {package} escapes the selected SDK"));
    }
    let manifest = Manifest::read(&directory, host, false)?;
    if manifest.name != package {
        return Err(format!(
            "SDK package {package} contains pubspec name {}",
            manifest.name
        ));
    }
    if manifest
        .dependencies
        .values()
        .any(|dependency| !matches!(&dependency.source, Source::Sdk(name) if name == "dart"))
    {
        return Err(format!(
            "SDK package {package} may only declare SDK packages as regular dependencies"
        ));
    }
    Ok(Candidate {
        manifest,
        manifest_error: None,
        directory,
        archive: None,
        digest: None,
    })
}

/// Reads the selected SDK's portable, root-relative vendored package mapping.
pub(super) fn declared_package_paths(root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let config_path = root.join("sdk_packages.yaml");
    let config = yaml_json(
        &std::fs::read_to_string(&config_path)
            .map_err(|error| format!("{}: {error}", config_path.display()))?,
    )?;
    if config["version"].as_i64() != Some(1) || config["sdk"].as_str() != Some("dart") {
        return Err("unsupported selected SDK package configuration".into());
    }
    let mut packages = BTreeMap::<String, PathBuf>::new();
    for row in config["packages"]
        .as_array()
        .ok_or("SDK packages must be a list")?
    {
        let name = row["name"]
            .as_str()
            .filter(|name| valid_name(name))
            .ok_or("SDK package name is invalid")?;
        let path = row["path"]
            .as_str()
            .filter(|path| !path.is_empty())
            .ok_or("SDK package path is missing")?;
        let path = Path::new(path);
        if path.as_os_str().to_string_lossy().contains(['\\', ':'])
            || path.components().any(|part| !matches!(part, std::path::Component::Normal(_)))
            || path.components().next().is_none()
        {
            return Err(format!("SDK package {name} must use a relative path without traversal"));
        }
        let relative: PathBuf = path.components().collect();
        if packages.insert(name.into(), relative).is_some() {
            return Err(format!("duplicate declared SDK package {name}"));
        }
    }
    Ok(packages)
}
