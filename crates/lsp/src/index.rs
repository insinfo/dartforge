//! Paths and names for the `import` directives the server writes (completion
//! of not-imported names, the "import library" fixes, moving files).

use std::path::Path;

/// Path of `target` relative to directory `base`, with `/`.
pub(crate) fn relative(target: &Path, base: &Path) -> Option<String> {
    use std::path::Component;
    let d: Vec<Component> = target.components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = d.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return None;
    }
    let mut parts: Vec<String> = b[common..].iter().map(|_| "..".to_string()).collect();
    parts.extend(
        d[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(parts.join("/"))
}

/// Package name in the root `pubspec.yaml`.
pub(crate) fn package_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("pubspec.yaml")).ok()?;
    text.lines().find_map(|l| {
        l.strip_prefix("name:")
            .map(|n| n.trim().trim_matches(['\'', '"']).to_string())
    })
}

/// The URI with which `file` imports `target` from the same project:
/// `package:` when `file` is outside `lib/` and `target` is inside; relative
/// otherwise.
pub(crate) fn import_uri(
    file: &Path,
    target: &Path,
    root: &Path,
    package: Option<&str>,
) -> Option<String> {
    let lib = root.join("lib");
    match package {
        Some(p) if target.starts_with(&lib) && !file.starts_with(&lib) => {
            relative(target, &lib).map(|r| format!("package:{p}/{r}"))
        }
        _ => file.parent().and_then(|dir| relative(target, dir)),
    }
}
