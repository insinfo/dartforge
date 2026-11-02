//! The functions of `package:path` (POSIX style) that Pub's publishing code
//! uses, on strings, so that paths are spelled in messages exactly as Pub
//! spells them (`p.join('.', 'lib')` is `./lib`, not `lib`).

/// `p.join(a, b)`.
pub(crate) fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        return b.to_owned();
    }
    if b.is_empty() {
        return a.to_owned();
    }
    if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// `p.split(path)`.
pub(crate) fn split(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    if path.starts_with('/') {
        parts.push("/".to_owned());
    }
    parts.extend(path.split('/').filter(|s| !s.is_empty()).map(str::to_owned));
    parts
}

/// `p.normalize(path)`.
pub(crate) fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_owned()
    } else {
        joined
    }
}

/// The working directory as `p.current` sees it.
pub(crate) fn current() -> String {
    std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_else(|_| "/".to_owned())
}

/// `p.absolute(path)` (not normalized).
pub(crate) fn absolute(path: &str) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        join(&current(), path)
    }
}

/// `p.canonicalize(path)` on POSIX: absolute and normalized.
pub(crate) fn canonicalize(path: &str) -> String {
    normalize(&absolute(path))
}

/// `p.relative(path, from: from)`.
pub(crate) fn relative(path: &str, from: &str) -> String {
    let path = canonicalize(path);
    let from = canonicalize(from);
    let a: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; b.len() - common];
    parts.extend(&a[common..]);
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// `p.relative(path)` from the working directory.
pub(crate) fn relative_to_current(path: &str) -> String {
    relative(path, &current())
}

/// `p.basename(path)`.
pub(crate) fn basename(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.starts_with('/') {
            "/".into()
        } else {
            String::new()
        };
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_owned()
}

/// `p.dirname(path)`.
pub(crate) fn dirname(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        None => {
            if path.starts_with('/') {
                "/".into()
            } else {
                ".".into()
            }
        }
        Some(index) => {
            let parent = trimmed[..index].trim_end_matches('/');
            if parent.is_empty() {
                "/".into()
            } else {
                parent.to_owned()
            }
        }
    }
}

/// `p.extension(path)`: from the last `.` of the basename, unless that is
/// its first character.
pub(crate) fn extension(path: &str) -> String {
    let base = basename(path);
    if base == ".." {
        return String::new();
    }
    match base.rfind('.') {
        Some(index) if index > 0 => base[index..].to_owned(),
        _ => String::new(),
    }
}

/// `p.basenameWithoutExtension(path)`.
pub(crate) fn basename_without_extension(path: &str) -> String {
    let base = basename(path);
    let ext = extension(path);
    base[..base.len() - ext.len()].to_owned()
}

/// `p.isWithin(parent, child)`.
pub(crate) fn is_within(parent: &str, child: &str) -> bool {
    let parent = canonicalize(parent);
    let child = canonicalize(child);
    if parent == "/" {
        return child != "/";
    }
    child.len() > parent.len()
        && child.starts_with(&parent)
        && child.as_bytes()[parent.len()] == b'/'
}

/// `p.equals(a, b)`.
pub(crate) fn equals(a: &str, b: &str) -> bool {
    canonicalize(a) == canonicalize(b)
}

/// The `path` of `p.toUri(relativePath)`: the path percent-encoded like a
/// URI path.
pub(crate) fn to_uri_path(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        let c = byte as char;
        if c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:@/".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaves_like_package_path() {
        assert_eq!(join(".", "lib"), "./lib");
        assert_eq!(join("a/", "b"), "a/b");
        assert_eq!(join("a", "/b"), "/b");
        assert_eq!(normalize("./a/../b//c/."), "b/c");
        assert_eq!(normalize("."), ".");
        assert_eq!(normalize("../a"), "../a");
        assert_eq!(normalize("/a/../.."), "/");
        assert_eq!(split("/a/b"), ["/", "a", "b"]);
        assert_eq!(split("a/b/"), ["a", "b"]);
        assert_eq!(relative("/a/b/c", "/a/d"), "../b/c");
        assert_eq!(relative("/a", "/a"), ".");
        assert_eq!(basename("a/b.dart"), "b.dart");
        assert_eq!(dirname("a/b.dart"), "a");
        assert_eq!(dirname("b.dart"), ".");
        assert_eq!(dirname("/b"), "/");
        assert_eq!(extension("a/.gitignore"), "");
        assert_eq!(extension("a/b.tar.gz"), ".gz");
        assert_eq!(basename_without_extension("lib/x.dart"), "x");
        assert!(is_within("/a", "/a/b"));
        assert!(!is_within("/a", "/ab"));
        assert!(!is_within("/a", "/a"));
        assert_eq!(to_uri_path("a b/c"), "a%20b/c");
    }
}
