//! The analyzer's `PubspecValidator` (`analyzer/lib/src/pubspec/`, read in
//! 6.11.0; docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.10): the seven
//! sub-validators of `validatePubspec`, in the original's order
//! (dependencies, fields, flutter, name, screenshots, platforms, workspace).
//! All are warnings, on the YAML node's range.
//!
//! Not covered: the pubspec lints and the `# ignore` filter at the end of
//! `validatePubspec`. The existence of files is checked on disk.
//! Written without compiling or running (2026-10-05).

use super::codigos_g::pubspec as c;
use super::yaml::{self, No, Valor};
use super::Report;
use dartforge_diagnostics::Span;
use std::path::{Path, PathBuf};

/// `joinAll(posix.split(p))` from the pubspec's folder.
fn path_in(folder: &Path, relative: &str) -> PathBuf {
    let mut out = folder.to_path_buf();
    if relative.starts_with('/') {
        out = PathBuf::from("/");
    }
    for part in relative.split('/').filter(|p| !p.is_empty()) {
        out.push(part);
    }
    out
}

/// The path without `.` and `..` (`normalize`), without consulting the disk.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in p.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

struct Ctx<'a> {
    pasta: &'a Path,
    out: Vec<Report>,
}

impl Ctx<'_> {
    fn report(&mut self, code: &'static super::CodigoNaoDart, node: &No, args: &[&str]) {
        self.out.push(Report::new(code, node.span, args));
    }

    /// `validatePathEntries`.
    fn path_entries(&mut self, dependency: &No, check: bool) {
        if dependency.mapa().is_none() {
            return;
        }
        if let Some((key, value)) = dependency.par("path")
            && let Some(p) = value.texto()
        {
            if p.contains('\\') {
                self.report(&c::PATH_NOT_POSIX, value, &[p]);
                return;
            }
            let folder = normalize(&path_in(self.pasta, p));
            if !folder.is_dir() {
                self.report(&c::PATH_DOES_NOT_EXIST, value, &[p]);
            } else if !folder.join("pubspec.yaml").is_file() {
                self.report(&c::PATH_PUBSPEC_DOES_NOT_EXIST, value, &[p]);
            }
            if check {
                self.report(&c::INVALID_DEPENDENCY, key, &["path"]);
            }
        }
        if let Some((key, value)) = dependency.par("git")
            && !value.nulo()
            && check
        {
            self.report(&c::INVALID_DEPENDENCY, key, &["git"]);
        }
    }

    /// `getDeclaredDependencies`: absent or null, nothing; a map, its pairs;
    /// anything else, the warning.
    fn declared_dependencies<'n>(&mut self, root: &'n No, field: &str) -> &'n [(No, No)] {
        match root.campo(field) {
            None => &[],
            Some(v) if v.nulo() => &[],
            Some(v) => match v.mapa() {
                Some(m) => m,
                None => {
                    self.report(&c::DEPENDENCIES_FIELD_NOT_MAP, v, &[field]);
                    &[]
                }
            },
        }
    }

    /// `dependencyValidator`.
    fn validate_dependencies(&mut self, root: &No) {
        let normais = self.declared_dependencies(root, "dependencies");
        let dev = self.declared_dependencies(root, "dev_dependencies");
        let publicavel = root.campo("version").is_some_and(|v| !v.nulo()) && root.campo("publish_to").and_then(No::texto) != Some("none");
        for (_, value) in normais {
            self.path_entries(value, publicavel);
        }
        for (key, value) in dev {
            if let Some(name) = key.texto()
                && normais.iter().any(|(k, _)| k.texto() == Some(name))
            {
                self.report(&c::UNNECESSARY_DEV_DEPENDENCY, key, &[name]);
            }
            self.path_entries(value, false);
        }
    }

    /// `fieldValidator`: the top-level keys no longer in use.
    fn validate_fields(&mut self, root: &No) {
        for (key, _) in root.mapa().unwrap_or(&[]) {
            if let Some(name) = key.texto()
                && ["author", "authors", "transformers", "web"].contains(&name)
            {
                self.report(&c::DEPRECATED_FIELD, key, &[name]);
            }
        }
    }

    /// `_assetExistsAtPath`: a folder, a file, or the file in any subfolder of
    /// the parent (the resolution variants).
    fn asset_exists(&self, p: &Path) -> bool {
        if p.is_dir() || p.is_file() {
            return true;
        }
        let (Some(name), Some(parent)) = (p.file_name(), p.parent()) else { return false };
        let Ok(children) = std::fs::read_dir(parent) else { return false };
        children.flatten().any(|f| f.path().is_dir() && f.path().join(name).is_file())
    }

    /// `_validateAssetPath`.
    fn asset_path(&mut self, value: &str, node: &No) {
        if value.starts_with("packages/") {
            return;
        }
        if !self.asset_exists(&path_in(self.pasta, value)) {
            let code = if value.ends_with('/') { &c::ASSET_DIRECTORY_DOES_NOT_EXIST } else { &c::ASSET_DOES_NOT_EXIST };
            self.report(code, node, &[value]);
        }
    }

    /// `flutterValidator`.
    fn flutter(&mut self, root: &No) {
        let Some(flutter) = root.campo("flutter") else { return };
        if flutter.mapa().is_none() {
            // An empty `flutter:` is accepted.
            if !flutter.nulo() {
                self.report(&c::FLUTTER_FIELD_NOT_MAP, flutter, &[]);
            }
            return;
        }
        let Some(assets) = flutter.campo("assets") else { return };
        let Some(items) = assets.lista() else {
            self.report(&c::ASSET_FIELD_NOT_LIST, assets, &[]);
            return;
        };
        for item in items {
            if item.escalar() {
                // A scalar that is not a string interrupts the validator.
                let Some(entry) = item.texto() else {
                    self.report(&c::ASSET_NOT_STRING_OR_MAP, item, &[]);
                    return;
                };
                self.asset_path(entry, item);
            } else if item.mapa().is_some() {
                match item.campo("path") {
                    None => self.report(&c::ASSET_MISSING_PATH, item, &[]),
                    Some(p) if !p.escalar() => self.report(&c::ASSET_PATH_NOT_STRING, p, &[]),
                    Some(p) => {
                        let Some(entry) = p.texto() else {
                            self.report(&c::ASSET_NOT_STRING, p, &[]);
                            return;
                        };
                        self.asset_path(entry, p);
                    }
                }
            } else {
                self.report(&c::ASSET_NOT_STRING_OR_MAP, item, &[]);
            }
        }
    }

    /// `nameValidator`, for a document that is a map.
    fn validate_name(&mut self, root: &No) {
        match root.campo("name") {
            None => self.out.push(Report::new(&c::MISSING_NAME, Span { start: 0, end: 0 }, &[])),
            Some(n) if n.texto().is_none() => self.report(&c::NAME_NOT_STRING, n, &[]),
            Some(_) => {}
        }
    }

    /// `screenshotsValidator`: the file of each `path:` exists.
    fn validate_screenshots(&mut self, root: &No) {
        let Some(items) = root.campo("screenshots").and_then(No::lista) else { return };
        for item in items {
            if let Some(p) = item.campo("path")
                && let Some(text) = p.texto()
                && !path_in(self.pasta, text).is_file()
            {
                self.report(&c::PATH_DOES_NOT_EXIST, p, &[text]);
            }
        }
    }

    /// `platformsValidator`.
    fn validate_platforms(&mut self, root: &No, source: &str) {
        let Some(plataformas) = root.campo("platforms") else { return };
        let Some(pairs) = plataformas.mapa() else {
            self.report(&c::INVALID_PLATFORMS_FIELD, plataformas, &[]);
            return;
        };
        for (key, _) in pairs {
            let known = key.texto().is_some_and(|t| ["android", "ios", "linux", "macos", "web", "windows"].contains(&t));
            if !known {
                // The string, the number, or the node as written.
                let shown = match &key.valor {
                    Valor::Texto(t) | Valor::Outro(t) => t.clone(),
                    _ => source.get(key.span.start..key.span.end).unwrap_or("").to_string(),
                };
                self.report(&c::UNKNOWN_PLATFORM, key, &[shown.as_str()]);
            }
        }
        for (_, value) in pairs {
            if !value.nulo() {
                self.report(&c::PLATFORM_VALUE_DISALLOWED, value, &[]);
            }
        }
    }

    /// `workspaceValidator`.
    fn workspace(&mut self, root: &No) {
        let Some(field) = root.campo("workspace") else { return };
        let Some(items) = field.lista() else {
            self.report(&c::WORKSPACE_FIELD_NOT_LIST, field, &[]);
            return;
        };
        for item in items {
            if !item.escalar() {
                self.report(&c::WORKSPACE_VALUE_NOT_STRING, item, &[]);
                continue;
            }
            let Some(entry) = item.texto() else {
                self.report(&c::WORKSPACE_VALUE_NOT_STRING, item, &[]);
                return;
            };
            // `_validateDirectoryPath`: a subfolder of the root, and existing.
            let folder = normalize(&path_in(self.pasta, entry));
            let package_root = normalize(self.pasta);
            if folder == package_root || !folder.starts_with(&package_root) {
                let text = self.pasta.to_string_lossy().into_owned();
                self.report(&c::WORKSPACE_VALUE_NOT_SUBDIRECTORY, item, &[text.as_str()]);
            } else if !folder.is_dir() {
                self.report(&c::PATH_DOES_NOT_EXIST, item, &[entry]);
            }
        }
    }
}

/// `validatePubspec`: the reports of the `pubspec.yaml` with text `source`
/// that lives in folder `folder`. A YAML syntax error is not reported here
/// (the server treats the document as an empty map): only the name is missing.
pub fn validate(source: &str, folder: &Path) -> Vec<Report> {
    let mut ctx = Ctx { pasta: folder, out: Vec::new() };
    let root = match yaml::ler(source) {
        Ok(Some(node)) if node.mapa().is_some() => node,
        // A document that is not a map: only `missing_name`, at 0/0.
        _ => return vec![Report::new(&c::MISSING_NAME, Span { start: 0, end: 0 }, &[])],
    };
    ctx.validate_dependencies(&root);
    ctx.validate_fields(&root);
    ctx.flutter(&root);
    ctx.validate_name(&root);
    ctx.validate_screenshots(&root);
    ctx.validate_platforms(&root, source);
    ctx.workspace(&root);
    ctx.out
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(code name, range text)` of each report.
    fn findings(source: &str) -> Vec<(&'static str, String)> {
        // A folder that does not exist: every relative path is missing.
        let folder = Path::new("pasta-que-nao-existe-para-o-teste");
        validate(source, folder).into_iter().map(|r| (r.code.nome, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn nome_e_campos() {
        assert_eq!(findings("version: 1.0.0\n"), vec![("missing_name", String::new())]);
        assert_eq!(findings("name: 3\n"), vec![("name_not_string", "3".to_string())]);
        assert_eq!(findings("name: a\nauthor: eu\n"), vec![("deprecated_field", "author".to_string())]);
        assert_eq!(findings("- a\n"), vec![("missing_name", String::new())]);
    }

    #[test]
    fn dependencies() {
        let source = "name: a\nversion: 1.0.0\ndependencies:\n  b:\n    path: ../b\n  c:\n    git: https://x\ndev_dependencies:\n  b: any\n";
        assert_eq!(
            findings(source),
            vec![
                ("path_does_not_exist", "../b".to_string()),
                ("invalid_dependency", "path".to_string()),
                ("invalid_dependency", "git".to_string()),
                ("unnecessary_dev_dependency", "b".to_string()),
            ]
        );
        assert_eq!(findings("name: a\ndependencies: 3\n"), vec![("dependencies_field_not_map", "3".to_string())]);
        // With a backslash, only `path_not_posix`.
        assert_eq!(findings("name: a\ndependencies:\n  b:\n    path: ..\\b\n"), vec![("path_not_posix", "..\\b".to_string())]);
    }

    #[test]
    fn plataformas_e_workspace() {
        assert_eq!(
            findings("name: a\nplatforms:\n  android:\n  fuchsia: x\n"),
            vec![("unknown_platform", "fuchsia".to_string()), ("platform_value_disallowed", "x".to_string())]
        );
        assert_eq!(findings("name: a\nworkspace: x\n"), vec![("workspace_field_not_list", "x".to_string())]);
    }
}
