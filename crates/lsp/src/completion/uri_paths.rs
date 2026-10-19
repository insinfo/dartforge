//! Decode file-completion lookup paths without rewriting the editor's URI.
use std::path::{Path, PathBuf};
use url::Url;

pub(super) fn directory(source: &Path, parent: &str) -> Option<PathBuf> {
    // Dart's Uri.toFilePath rejects encoded path separators. Decoding those
    // through Url alone would silently turn one URI segment into two folders.
    let encoded = parent.to_ascii_lowercase();
    if encoded.contains("%2f") || cfg!(windows) && encoded.contains("%5c") {
        return None;
    }
    let authority = parent
        .strip_prefix("file://")
        .or_else(|| parent.strip_prefix("//"))
        .map(|tail| tail.split('/').next().unwrap_or_default());
    if !cfg!(windows) && authority.is_some_and(|host| !host.is_empty()) {
        return None;
    }
    let directory = if parent.starts_with("file://") {
        Url::parse(parent).ok()?
    } else if cfg!(windows) && Path::new(parent).is_absolute() {
        let normalized = parent.replace('\\', "/");
        let uri = if normalized.starts_with("//") {
            format!("file:{normalized}")
        } else {
            format!("file:///{normalized}")
        };
        Url::parse(&uri).ok()?
    } else {
        Url::from_directory_path(source.parent()?)
            .ok()?
            .join(parent)
            .ok()?
    };
    if directory.query().is_some() || directory.fragment().is_some() {
        return None;
    }
    let path = directory.to_file_path().ok()?;
    // Dart decodes URI segments as UTF-8 before building a filesystem path.
    path.to_str()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_decode_once_and_keep_the_source_directory() {
        let root = std::env::temp_dir().join("uri lookup");
        let source = root.join("main.dart");
        for (uri, name) in [
            ("folder%20space/", "folder space"),
            ("hash%23tag/", "hash#tag"),
            ("percent%25literal/", "percent%literal"),
            ("encoded%2520name/", "encoded%20name"),
            ("literal%zz/", "literal%zz"),
        ] {
            assert_eq!(directory(&source, uri), Some(root.join(name)), "{uri}");
        }
    }

    #[test]
    fn uri_components_and_encoded_separators_are_not_filesystem_paths() {
        let source = std::env::temp_dir().join("main.dart");
        for uri in [
            "folder%2Fchild/",
            "folder%2fchild/",
            "folder?query/",
            "folder#fragment/",
            "folder%FF/",
        ] {
            assert!(directory(&source, uri).is_none(), "{uri}");
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn non_windows_file_authorities_are_rejected_like_dart() {
        let source = Path::new("/project/lib/main.dart");
        for uri in [
            "file://localhost/project/lib/",
            "file://server/project/lib/",
        ] {
            assert!(directory(source, uri).is_none(), "{uri}");
        }
    }
}
