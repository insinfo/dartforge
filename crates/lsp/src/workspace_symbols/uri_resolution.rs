//! Analysis-server URI resolution, before generated-file substitution.
//!
//! Dot-segment removal adapts Dart SDK 3.6.2 sdk/lib/core/uri.dart
//! (Copyright 2012, the Dart project authors). BSD license and attribution:
//! THIRD_PARTY_NOTICES.md.

use super::{Context, SdkFiles};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Context loading normalizes roots once. General-purpose callers can still
/// supply roots containing dot segments or Windows verbatim prefixes.
pub(super) fn catalog_path(path: &Path, normalized: bool) -> Cow<'_, Path> {
    if normalized {
        Cow::Borrowed(path)
    } else {
        dartforge_elements::generated::key_of(path)
    }
}

/// Canonical bases depend on the referring context, not on target existence.
/// Keep them for one query only; every target still checks generated outputs.
#[derive(Default)]
pub(super) struct RelativeBases {
    values: RefCell<HashMap<(usize, Arc<PathBuf>), RelativeBase>>,
}

impl RelativeBases {
    pub(super) fn target(
        &self,
        context: usize,
        unit: &Arc<PathBuf>,
        uri: &str,
        sdk: Option<&SdkFiles>,
        catalog: &Context,
    ) -> Option<PathBuf> {
        resolve_reference(
            uri,
            sdk,
            &catalog.packages,
            &catalog.generated,
            &catalog.generated_files,
            |reference| {
                let mut values = self.values.borrow_mut();
                let base = values.entry((context, unit.clone())).or_insert_with(|| {
                    if catalog.normalized_paths {
                        RelativeBase::new::<true>(
                            unit,
                            &catalog.packages,
                            &catalog.generated,
                            &catalog.generated_files,
                        )
                    } else {
                        RelativeBase::new::<false>(
                            unit,
                            &catalog.packages,
                            &catalog.generated,
                            &catalog.generated_files,
                        )
                    }
                });
                base.target(
                    reference,
                    &catalog.packages,
                    &catalog.generated,
                    &catalog.generated_files,
                )
            },
        )
    }
}

enum RelativeBase {
    Package { name: String, path: String },
    File(Option<url::Url>),
}

impl RelativeBase {
    fn new<const NORMALIZED: bool>(
        unit: &Path,
        packages: &[(String, PathBuf)],
        generated: &[(String, PathBuf, PathBuf)],
        generated_files: &[(PathBuf, PathBuf)],
    ) -> Self {
        let source_unit = generated
            .iter()
            .map(|(_, source, output)| (source, output))
            .chain(
                generated_files
                    .iter()
                    .map(|(source, output)| (source, output)),
            )
            .find_map(|(source, output)| {
                let output = catalog_path(output, NORMALIZED);
                unit.strip_prefix(output.as_ref()).ok().map(|relative| {
                    let source = source.join(relative);
                    if NORMALIZED {
                        source
                    } else {
                        dartforge_elements::generated::key(&source)
                    }
                })
            });
        let source_unit = source_unit.as_deref().unwrap_or(unit);
        for (name, lib) in packages {
            let lib = catalog_path(lib, NORMALIZED);
            if let Ok(relative) = source_unit.strip_prefix(lib.as_ref()) {
                return Self::Package {
                    name: name.clone(),
                    path: package_base(relative),
                };
            }
        }
        Self::File(url::Url::from_file_path(source_unit).ok())
    }

    fn target(
        &self,
        reference: &str,
        packages: &[(String, PathBuf)],
        generated: &[(String, PathBuf, PathBuf)],
        generated_files: &[(PathBuf, PathBuf)],
    ) -> Option<PathBuf> {
        match self {
            Self::Package { name, path } => package_target(
                &relative_package_path(name, path, reference),
                packages,
                generated,
            ),
            Self::File(base) => {
                let target = base.as_ref()?.join(reference).ok()?.to_file_path().ok()?;
                Some(file_target(&target, generated_files))
            }
        }
    }
}

/// Resolve relative imports from their canonical package URI. Package roots
/// clamp relative parent traversal; ordinary file URIs retain filesystem roots.
pub(super) fn dependency_target(
    unit: &Path,
    uri: &str,
    sdk: Option<&SdkFiles>,
    packages: &[(String, PathBuf)],
    generated: &[(String, PathBuf, PathBuf)],
    generated_files: &[(PathBuf, PathBuf)],
) -> Option<PathBuf> {
    resolve_reference(
        uri,
        sdk,
        packages,
        generated,
        generated_files,
        |reference| {
            RelativeBase::new::<false>(unit, packages, generated, generated_files).target(
                reference,
                packages,
                generated,
                generated_files,
            )
        },
    )
}

fn resolve_reference(
    uri: &str,
    sdk: Option<&SdkFiles>,
    packages: &[(String, PathBuf)],
    generated: &[(String, PathBuf, PathBuf)],
    generated_files: &[(PathBuf, PathBuf)],
    relative: impl FnOnce(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let (reference, fragment) = uri.split_once('#').unwrap_or((uri, ""));
    // Dart Uri.isAbsolute excludes a nonempty fragment. SourceFactory rejects
    // such references, including package URIs whose files otherwise exist.
    if !fragment.is_empty() {
        return None;
    }
    if let Ok(parsed) = url::Url::parse(reference) {
        return match parsed.scheme() {
            "dart" => sdk?.layout.library(parsed.path()).map(|l| l.path.clone()),
            "package" => {
                let raw = reference.split_once(':')?.1;
                package_target(&absolute_path(raw), packages, generated)
            }
            "file" => Some(file_target(&parsed.to_file_path().ok()?, generated_files)),
            _ => None,
        };
    }
    relative(reference)
}

fn package_base(relative: &Path) -> String {
    normalized_path(&format!(
        "/{}",
        relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    ))
}

/// Merge below the package name before RFC dot-segment removal. This is
/// Uri.resolveUri's package branch, independent of physical source paths.
fn relative_package_path(name: &str, base: &str, reference: &str) -> String {
    if reference.starts_with("//") {
        return absolute_path(reference);
    }
    let reference = normalized_path(reference.split('?').next().unwrap_or_default());
    let merged = if reference.is_empty() {
        base.to_owned()
    } else if reference.starts_with('/') {
        reference
    } else {
        format!("{}{reference}", &base[..=base.rfind('/').unwrap()])
    };
    format!("{name}{}", remove_dot_segments(&merged))
}

/// The normal package resolver decodes path segments. Its generated fallback
/// instead uses Uri.path verbatim, including percent escapes (SDK pub.dart).
fn package_target(
    path: &str,
    packages: &[(String, PathBuf)],
    generated: &[(String, PathBuf, PathBuf)],
) -> Option<PathBuf> {
    let segments = path.strip_prefix('/').unwrap_or(path);
    let (name, relative) = segments.split_once('/')?;
    let name = percent_encoding::percent_decode_str(name)
        .decode_utf8()
        .ok()?;
    let (_, lib) = packages.iter().find(|(n, _)| n == name.as_ref())?;
    let decoded = percent_encoding::percent_decode_str(relative)
        .decode_utf8()
        .ok()?;
    let source = lib.join(decoded.as_ref());
    let generated = generated.iter().find(|(n, ..)| n == name.as_ref());
    // A normal package URI with no generated counterpart needs no existence
    // check: the SDK still creates a FileState when that source is missing.
    if generated.is_none() && !path.starts_with('/') {
        return Some(source);
    }
    if source.is_file() {
        return Some(source);
    }
    // The basic resolver may accept an existing /package/path; the generated
    // resolver requires a package name before the first slash.
    if path.starts_with('/') {
        return None;
    }
    if let Some((_, _, output)) = generated {
        let target = output.join(relative);
        if target.is_file() {
            return Some(target);
        }
    }
    Some(source)
}

/// File URI resolution in a Pub workspace prefers the built file, even when
/// the source exists. Only mappings for packages inside that workspace apply.
fn file_target(target: &Path, generated_files: &[(PathBuf, PathBuf)]) -> PathBuf {
    for (source, output) in generated_files {
        let source = dartforge_elements::generated::key_of(source);
        if let Ok(relative) = target.strip_prefix(source.as_ref()) {
            let generated = output.join(relative);
            if generated.is_file() {
                return generated;
            }
        }
    }
    target.to_path_buf()
}

fn absolute_path(reference: &str) -> String {
    let reference = reference.split('?').next().unwrap_or_default();
    let path = if let Some(authority) = reference.strip_prefix("//") {
        authority.find('/').map_or("", |i| &authority[i..])
    } else {
        reference
    };
    remove_dot_segments(&normalized_path(path))
}

/// Canonical URI path escapes: unreserved bytes are literal, other escapes
/// use upper-case hex, malformed percent signs and UTF-8 bytes are escaped.
fn normalized_path(path: &str) -> String {
    fn unreserved(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
    }
    fn hex(byte: u8) -> Option<u8> {
        (byte as char).to_digit(16).map(|n| n as u8)
    }
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let mut byte = bytes[i];
        if byte == b'%'
            && i + 2 < bytes.len()
            && let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            byte = high * 16 + low;
            if unreserved(byte) {
                output.push(char::from(byte));
            } else {
                output.push('%');
                output.push(char::from(DIGITS[high as usize]));
                output.push(char::from(DIGITS[low as usize]));
            }
            i += 3;
            continue;
        }
        if byte == b'\\' {
            output.push('/');
        } else if unreserved(byte) || b"/!$&'()*+,;=:@".contains(&byte) {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(DIGITS[(byte >> 4) as usize]));
            output.push(char::from(DIGITS[(byte & 15) as usize]));
        }
        i += 1;
    }
    output
}

/// Dart's RFC dot-segment removal preserves empty segments and a trailing
/// slash. A package-relative path is merged after its package-name boundary.
fn remove_dot_segments(path: &str) -> String {
    if !path.starts_with('.') && !path.contains("/.") {
        return path.to_owned();
    }
    let mut output = Vec::new();
    let mut append_slash = false;
    for segment in path.split('/') {
        append_slash = false;
        match segment {
            ".." => {
                if !output.is_empty() {
                    output.pop();
                    if output.is_empty() {
                        output.push("");
                    }
                }
                append_slash = true;
            }
            "." => append_slash = true,
            other => output.push(other),
        }
    }
    if append_slash {
        output.push("");
    }
    output.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_catalogs_match_general_resolution_of_unnormalized_roots() {
        let root = super::super::tests::fixture("normalized-catalog");
        let lib = root.join("lib");
        let output = root.join(".dart_tool/build/generated/p/lib");
        for folder in [&lib, &output] {
            std::fs::create_dir_all(folder.join("src")).unwrap();
            std::fs::write(folder.join("target.dart"), "class Target {}").unwrap();
        }
        std::fs::write(
            root.join(".dart_tool/package_config.json"),
            serde_json::json!({"configVersion": 2, "packages": [{
                "name": "p", "rootUri": "../", "packageUri": "lib/./", "languageVersion": "3.6"
            }]})
            .to_string(),
        )
        .unwrap();
        let loaded = Context::load(&root);
        assert!(loaded.normalized_paths);
        let raw = Context {
            packages: vec![("p".into(), lib.join("../lib"))],
            generated: vec![("p".into(), lib.join("../lib"), output.join("../lib"))],
            generated_files: vec![(root.clone(), root.join(".dart_tool/build/generated/p"))],
            ..Context::default()
        };
        let bases = RelativeBases::default();
        let file_uri = url::Url::from_file_path(lib.join("target.dart")).unwrap();
        for unit in [lib.join("src/entry.dart"), output.join("src/entry.dart")] {
            assert_eq!(
                loaded.identity(&unit),
                super::super::FileIdentity::Package("p".into(), PathBuf::from("src/entry.dart"))
            );
            let unit = Arc::new(unit);
            for reference in [
                "../target.dart",
                "../../target.dart",
                "package:p/%74arget.dart?query",
                "../target.dart#fragment",
                file_uri.as_str(),
            ] {
                let normalize = |p: PathBuf| dartforge_elements::generated::key(&p);
                assert_eq!(
                    bases
                        .target(0, &unit, reference, None, &loaded)
                        .map(normalize),
                    bases.target(1, &unit, reference, None, &raw).map(normalize),
                    "{}: {reference}",
                    unit.display()
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shared_physical_units_keep_the_base_of_each_referring_context() {
        let root = super::super::tests::fixture("relative-base-contexts");
        let outer = root.join("lib");
        let inner = outer.join("nested");
        let unit = Arc::new(inner.join("entry.dart"));
        let contexts = [
            Context {
                packages: vec![("p".into(), outer.clone())],
                ..Context::default()
            },
            Context {
                packages: vec![("p".into(), inner.clone())],
                ..Context::default()
            },
        ];
        let bases = RelativeBases::default();
        for context in [0, 1, 0, 1] {
            let catalog = &contexts[context];
            let lib = if context == 0 { &outer } else { &inner };
            assert_eq!(
                bases.target(context, &unit, "../target.dart", None, catalog),
                Some(lib.join("target.dart"))
            );
            assert_eq!(
                bases.target(context, &unit, "./next.dart", None, catalog),
                Some(inner.join("next.dart"))
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cached_bases_do_not_freeze_generated_target_selection() {
        let root = super::super::tests::fixture("relative-base-generated");
        let lib = root.join("lib");
        let output = root.join("output");
        std::fs::create_dir_all(lib.join("src")).unwrap();
        std::fs::create_dir_all(output.join("src")).unwrap();
        let unit = Arc::new(output.join("src/entry.dart"));
        let source = lib.join("target.dart");
        let built = output.join("target.dart");
        let mut catalog = Context {
            packages: vec![("p".into(), lib.clone())],
            generated: vec![("p".into(), lib.clone(), output.clone())],
            generated_files: vec![(lib, output)],
            ..Context::default()
        };
        for normalized in [false, true] {
            catalog.normalized_paths = normalized;
            let bases = RelativeBases::default();
            let target = || bases.target(0, &unit, "../target.dart", None, &catalog);
            assert_eq!(target(), Some(source.clone()));
            std::fs::write(&built, "class Built {}").unwrap();
            assert_eq!(target(), Some(built.clone()));
            std::fs::write(&source, "class Source {}").unwrap();
            assert_eq!(target(), Some(source.clone()));
            // File URIs keep their different precedence, even for this same unit.
            let uri = url::Url::from_file_path(&source).unwrap();
            assert_eq!(
                bases.target(0, &unit, uri.as_str(), None, &catalog),
                Some(built.clone())
            );
            std::fs::remove_file(&source).unwrap();
            assert_eq!(target(), Some(built.clone()));
            std::fs::remove_file(&built).unwrap();
            assert_eq!(target(), Some(source.clone()));
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn package_paths_match_sixty_vectors_from_the_floor_vm() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/data/package_uri_paths.json")).unwrap();
        assert_eq!(reference["sdk"], "3.6.2");
        assert_eq!(reference["cases"].as_array().unwrap().len(), 60);
        for case in reference["cases"].as_array().unwrap() {
            let uri = case["reference"].as_str().unwrap();
            let actual = if case["kind"] == "absolute" {
                absolute_path(uri)
            } else {
                relative_package_path(
                    "p",
                    &package_base(Path::new(case["base"].as_str().unwrap())),
                    uri,
                )
            };
            assert_eq!(actual, case["expected"].as_str().unwrap(), "{case}");
        }
    }

    #[test]
    fn package_and_file_uris_keep_their_own_roots() {
        let root = super::super::tests::fixture("uri-roots");
        let lib = root.join("package/lib");
        std::fs::create_dir_all(lib.join("src")).unwrap();
        std::fs::write(lib.join("target.dart"), "class Correct {}").unwrap();
        std::fs::write(root.join("package/target.dart"), "class Decoy {}").unwrap();
        let packages = [("p".into(), lib.clone())];
        let unit = lib.join("src/entry.dart");
        for uri in [
            "../../target.dart",
            "/target.dart",
            "package:p/%74arget.dart?query",
        ] {
            assert_eq!(
                dependency_target(&unit, uri, None, &packages, &[], &[]),
                Some(lib.join("target.dart")),
                "{uri}"
            );
        }
        assert!(
            dependency_target(
                &unit,
                "package:p/target.dart#fragment",
                None,
                &packages,
                &[],
                &[]
            )
            .is_none()
        );
        assert_eq!(
            dependency_target(
                &root.join("plain.dart"),
                "package/target.dart",
                None,
                &packages,
                &[],
                &[]
            ),
            Some(root.join("package/target.dart"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_package_fallback_preserves_raw_escapes_and_file_uri_precedence() {
        let root = super::super::tests::fixture("uri-generated");
        let lib = root.join("lib");
        let output = root.join("output");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(lib.join("existing.dart"), "class Source {}").unwrap();
        std::fs::write(output.join("existing.dart"), "class Built {}").unwrap();
        std::fs::write(output.join("generated file.dart"), "class WrongEscape {}").unwrap();
        let packages = [("p".into(), lib.clone())];
        let generated = [("p".into(), lib.clone(), output.clone())];
        let files = [(lib.clone(), output.clone())];
        let unit = lib.join("entry.dart");
        assert_eq!(
            dependency_target(
                &unit,
                "package:p/existing.dart",
                None,
                &packages,
                &generated,
                &files
            ),
            Some(lib.join("existing.dart"))
        );
        let file_uri = url::Url::from_file_path(lib.join("existing.dart")).unwrap();
        assert_eq!(
            dependency_target(
                &unit,
                file_uri.as_str(),
                None,
                &packages,
                &generated,
                &files
            ),
            Some(output.join("existing.dart"))
        );
        assert_eq!(
            dependency_target(
                &unit,
                "package:p/generated%20file.dart",
                None,
                &packages,
                &generated,
                &files
            ),
            Some(lib.join("generated file.dart"))
        );
        std::fs::write(
            output.join("generated%20file.dart"),
            "class EncodedOutput {}",
        )
        .unwrap();
        assert_eq!(
            dependency_target(
                &unit,
                "package:p/generated%20file.dart",
                None,
                &packages,
                &generated,
                &files
            ),
            Some(output.join("generated%20file.dart"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
