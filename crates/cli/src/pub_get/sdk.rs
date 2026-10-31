//! Packages vendored by the Dart SDK (`sdk: dart`), from the selected SDK's
//! `sdk_packages.yaml`, as Pub's `DartSdk.packagePath` finds them.
use super::{manifest::valid_name, source::SourceError, tree::parse_yaml, version::Version};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub(super) fn version(root: &Path) -> Result<Version, String> {
    let path = root.join("version");
    Version::parse(
        std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .trim(),
    )
}

/// The directory of the SDK package `package`, which must belong to the SDK
/// whose version is `expected_version`.
pub(super) fn package_directory(
    root: Option<&Path>,
    package: &str,
    expected_version: &Version,
) -> Result<PathBuf, SourceError> {
    let not_found =
        || SourceError::NotFound(format!("could not find package {package} in the Dart SDK"));
    let Some(root) = root else {
        return Err(not_found());
    };
    if version(root)? != *expected_version {
        return Err(SourceError::Fatal(format!(
            "selected Dart SDK version differs from resolution SDK {expected_version}"
        )));
    }
    if !root.join("sdk_packages.yaml").is_file() {
        return Err(not_found());
    }
    let packages = declared_package_paths(root)?;
    let Some(relative) = packages.get(package) else {
        return Err(not_found());
    };
    let directory = root.join(relative);
    if !directory.is_dir() {
        return Err(not_found());
    }
    let canonical = directory
        .canonicalize()
        .map_err(|error| format!("SDK package {package}: {error}"))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("{}: {error}", root.display()))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(SourceError::Fatal(format!(
            "SDK package {package} escapes the selected SDK"
        )));
    }
    Ok(directory)
}

/// Reads the selected SDK's portable, root-relative vendored package mapping.
pub(crate) fn declared_package_paths(root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let config_path = root.join("sdk_packages.yaml");
    let config = parse_yaml(
        &std::fs::read_to_string(&config_path)
            .map_err(|error| format!("{}: {error}", config_path.display()))?,
    )?;
    if config.get("version").to_json() != "1" || config.get("sdk").as_str() != Some("dart") {
        return Err("unsupported selected SDK package configuration".into());
    }
    let mut packages = BTreeMap::<String, PathBuf>::new();
    for row in config
        .get("packages")
        .as_list()
        .ok_or("SDK packages must be a list")?
    {
        let name = row
            .get("name")
            .as_str()
            .filter(|name| valid_name(name))
            .ok_or("SDK package name is invalid")?;
        let path = row
            .get("path")
            .as_str()
            .filter(|path| !path.is_empty())
            .ok_or("SDK package path is missing")?;
        let path = Path::new(path);
        if path.as_os_str().to_string_lossy().contains(['\\', ':'])
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || path.components().next().is_none()
        {
            return Err(format!(
                "SDK package {name} must use a relative path without traversal"
            ));
        }
        let relative: PathBuf = path.components().collect();
        if packages.insert(name.into(), relative).is_some() {
            return Err(format!("duplicate declared SDK package {name}"));
        }
    }
    Ok(packages)
}
