use std::{
    io::{self, Error, ErrorKind},
    path::{Path, PathBuf},
};

/// A trait to allow replacing the file system lookup mechanisms.
///
/// As it stands, this is imperfect: it’s still using the types and some operations from
/// `std::path`, which constrain it to the target platform’s norms. This could be ameliorated by
/// the use of associated types for `Path` and `PathBuf`, and putting all remaining methods on this
/// trait (`is_absolute`, `parent`, `join`, *&c.*); but that would infect too many other APIs to be
/// desirable, so we live with it as it is—which is also acceptable, because the motivating example
/// use case is mostly using this as an optimisation over the real platform underneath.
pub trait Fs: std::fmt::Debug {
    /// Returns `true` if the path exists on disk and is pointing at a directory.
    fn is_dir(&self, path: &Path) -> bool;
    /// Returns `true` if the path exists on disk and is pointing at a regular file.
    fn is_file(&self, path: &Path) -> bool;
    /// Read the entire contents of a file into a bytes vector.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    /// Canonicalize a file path
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        Ok(path.to_path_buf())
    }
}

/// Use [`std::fs`] to read any files from disk.
///
/// This is the default file system implementation.
#[derive(Debug)]
pub struct StdFs;

impl Fs for StdFs {
    #[inline]
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    #[inline]
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    #[inline]
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    #[inline]
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::canonicalize(path).map(without_verbatim)
    }
}

/// The plain form of a Windows verbatim path (`\\?\C:\x` is `C:\x`,
/// `\\?\UNC\srv\share` is `\\srv\share`), which `std::fs::canonicalize`
/// returns there, so that the canonical paths of imports compare with the
/// paths the caller passes and show as the user wrote them. The same rule
/// as `dartforge_elements::config::without_verbatim` (this crate stays
/// without dependencies on the rest of DartForge): kept verbatim when the
/// plain form would name another file (over `MAX_PATH`, a reserved device
/// name, a component ending in a dot or a space, a `/`). A no-op elsewhere.
fn without_verbatim(p: PathBuf) -> PathBuf {
    let Some(s) = p.to_str() else {
        return p;
    };
    let plain = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
            return p;
        }
        rest.to_string()
    } else {
        return p;
    };
    if plain.len() >= 260 || plain.contains('/') {
        return p;
    }
    let reserved = |c: &str| {
        let stem = c
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end()
            .to_ascii_uppercase();
        matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    };
    for c in plain.trim_start_matches('\\').split('\\').skip(1) {
        if !c.is_empty() && (reserved(c) || c.ends_with('.') || c.ends_with(' ')) {
            return p;
        }
    }
    PathBuf::from(plain)
}

/// A file system implementation that acts like it’s completely empty.
///
/// This may be useful for security as it denies all access to the file system (so `@import` is
/// prevented from leaking anything); you’ll need to use [`from_string`][crate::from_string] for
/// this to make any sense (since [`from_path`][crate::from_path] would fail to find a file).
#[derive(Debug)]
pub struct NullFs;

impl Fs for NullFs {
    #[inline]
    fn is_file(&self, _path: &Path) -> bool {
        false
    }

    #[inline]
    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    #[inline]
    fn read(&self, _path: &Path) -> io::Result<Vec<u8>> {
        Err(Error::new(
            ErrorKind::NotFound,
            "NullFs, there is no file system",
        ))
    }
}
