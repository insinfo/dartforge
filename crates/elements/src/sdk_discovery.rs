//! Locating the Dart SDK's `lib/` on the machine, with a record of every
//! candidate looked at, for the tools that need an SDK without being told
//! where it is (the compiler, `dartforge analyze`, the language server).
//!
//! The order, first valid candidate wins:
//!
//! 1. `DARTFORGE_SDK_LIB` (the SDK's `lib/`, or its root);
//! 2. the SDK next to the running executable: the dartforge distribution
//!    (`<root>/lib/dart-sdk`, see [`crate::distribution`]), or a `dart-sdk/`
//!    beside the executable;
//! 3. `DART_SDK` (the SDK root, or its `lib/`);
//! 4. `FLUTTER_ROOT` (`<flutter>/bin/cache/dart-sdk`);
//! 5. `dart` on the `PATH`, following symbolic links (Homebrew) and the
//!    Flutter wrapper script (`<flutter>/bin/dart` → `bin/cache/dart-sdk`);
//! 6. `flutter` on the `PATH`, the same way;
//! 7. well-known install locations (Homebrew, Debian's package, Chocolatey,
//!    `~/flutter`, `~/fvm/default`…). Editors started from a desktop launcher
//!    often run without the user's shell `PATH`, which is why these exist.
//!
//! A candidate is valid when it has `libraries.json` and `core/core.dart`
//! and, with [`search_with`], when the caller manages to load it: a candidate
//! that exists but fails to load (a corrupt `libraries.json`, an unreadable
//! `core.dart`) is recorded as refused and the search goes on to the next one.
//!
//! The SDK's version does not change the order: an explicit choice
//! (`DARTFORGE_SDK_LIB`, `DART_SDK`…) wins even when a 3.6.x SDK exists further
//! down the list. The tools report the version of the chosen SDK, and the
//! language server warns when it is not 3.6.x.

use std::path::{Path, PathBuf};

/// The SDK chosen by [`search`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundSdk {
    /// The SDK's `lib/` directory.
    pub lib: PathBuf,
    /// Where it came from, for messages (`DART_SDK`, `dart on the PATH (/usr/bin/dart)`).
    pub source: String,
    /// The SDK's `version` file (`3.6.2`), when there is one next to `lib/`.
    pub version: Option<String>,
}

/// The outcome of [`search`]: the chosen SDK and the refused candidates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SdkSearch {
    /// The first valid candidate, if any.
    pub found: Option<FoundSdk>,
    /// Candidates looked at and refused before the chosen one (or all of them),
    /// in order, each as `<source>: <path> (<reason>)`. Locations that simply do
    /// not exist are not listed, except the ones the user named (environment
    /// variables), so that a typo is visible.
    pub rejected: Vec<String>,
}

/// Checks that `path` is a Dart SDK's `lib/` or the SDK root, and returns the `lib/`.
///
/// # Errors
/// A message naming what is missing.
///
/// ```
/// let dir = std::env::temp_dir().join(format!("sdk-doc-{}", std::process::id()));
/// std::fs::create_dir_all(dir.join("lib/core")).unwrap();
/// std::fs::write(dir.join("lib/libraries.json"), "{}").unwrap();
/// std::fs::write(dir.join("lib/core/core.dart"), "").unwrap();
/// let lib = dartforge_elements::sdk_discovery::sdk_lib(&dir).unwrap();
/// assert_eq!(lib, dir.join("lib"));
/// assert!(dartforge_elements::sdk_discovery::sdk_lib(&dir.join("missing")).is_err());
/// std::fs::remove_dir_all(&dir).unwrap();
/// ```
pub fn sdk_lib(path: &Path) -> Result<PathBuf, String> {
    let lib = if path.join("libraries.json").is_file() {
        path.to_path_buf()
    } else if path.join("lib").join("libraries.json").is_file() {
        path.join("lib")
    } else if !path.exists() {
        return Err("does not exist".to_string());
    } else {
        return Err("no libraries.json in it or in its lib/".to_string());
    };
    if !lib.join("core").join("core.dart").is_file() {
        return Err(format!("{} has no core/core.dart", lib.display()));
    }
    Ok(lib)
}

/// The `version` file of the SDK whose `lib/` is `lib` (`3.6.2`).
pub fn sdk_version(lib: &Path) -> Option<String> {
    let text = std::fs::read_to_string(lib.parent()?.join("version")).ok()?;
    let v = text.trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// Runs the discovery described in the module documentation.
pub fn search() -> SdkSearch {
    search_with(&mut |_| Ok(()))
}

/// [`search`], where a candidate is chosen only when `load` accepts its
/// `lib/` too; a refusal is listed in [`SdkSearch::rejected`] as
/// `<source>: <lib> (could not be loaded: <reason>)` and the next candidate
/// is tried. `load` typically loads the SDK and keeps it.
///
/// ```
/// // Nothing loads: every candidate on this machine is refused.
/// let search = dartforge_elements::sdk_discovery::search_with(&mut |_| Err("refused".into()));
/// assert!(search.found.is_none());
/// ```
pub fn search_with(load: &mut dyn FnMut(&Path) -> Result<(), String>) -> SdkSearch {
    let mut s = Search {
        found: None,
        rejected: Vec::new(),
        load,
    };
    s.run();
    SdkSearch {
        found: s.found,
        rejected: s.rejected,
    }
}

struct Search<'a> {
    found: Option<FoundSdk>,
    rejected: Vec<String>,
    /// The caller's check of a valid candidate ([`search_with`]).
    load: &'a mut dyn FnMut(&Path) -> Result<(), String>,
}

impl Search<'_> {
    /// Chooses `lib` (already checked by [`sdk_lib`]) if the caller loads it;
    /// otherwise records why not.
    fn accept(&mut self, source: &str, lib: PathBuf) -> bool {
        match (self.load)(&lib) {
            Ok(()) => {
                self.found = Some(FoundSdk {
                    version: sdk_version(&lib),
                    lib,
                    source: source.to_string(),
                });
                true
            }
            Err(e) => {
                self.rejected.push(format!(
                    "{source}: {} (could not be loaded: {e})",
                    lib.display()
                ));
                false
            }
        }
    }

    /// Tries `path` (an SDK root or `lib/`). `named` candidates (the user set
    /// them) are listed as refused even when they do not exist.
    fn probe(&mut self, source: &str, path: &Path, named: bool) -> bool {
        if self.found.is_some() {
            return true;
        }
        match sdk_lib(path) {
            Ok(lib) => self.accept(source, lib),
            Err(reason) => {
                if named || path.exists() {
                    self.rejected
                        .push(format!("{source}: {} ({reason})", path.display()));
                }
                false
            }
        }
    }

    fn run(&mut self) {
        if let Some(v) = non_empty_var("DARTFORGE_SDK_LIB")
            && self.probe("DARTFORGE_SDK_LIB", Path::new(&v), true)
        {
            return;
        }
        if self.next_to_executable() {
            return;
        }
        if let Some(v) = non_empty_var("DART_SDK")
            && self.probe("DART_SDK", Path::new(&v), true)
        {
            return;
        }
        if let Some(v) = non_empty_var("FLUTTER_ROOT")
            && self.probe("FLUTTER_ROOT", &flutter_sdk(Path::new(&v)), true)
        {
            return;
        }
        if self.on_path() {
            return;
        }
        for (source, path) in known_locations() {
            if self.probe(&source, &path, false) {
                return;
            }
        }
    }

    /// The distribution's SDK, or a `dart-sdk/` beside the executable (an
    /// editor extension may bundle one next to the language server).
    fn next_to_executable(&mut self) -> bool {
        if let Some(root) = crate::distribution::root()
            && self.probe(
                "the dartforge distribution",
                &root.join("lib").join("dart-sdk"),
                true,
            )
        {
            return true;
        }
        let Some(dir) = std::env::current_exe()
            .ok()
            .map(|e| e.canonicalize().unwrap_or(e))
            .and_then(|e| e.parent().map(Path::to_path_buf))
        else {
            return false;
        };
        let candidates = [
            dir.join("dart-sdk"),
            dir.join("..").join("lib").join("dart-sdk"),
        ];
        candidates
            .iter()
            .any(|c| self.probe("the SDK next to the executable", c, false))
    }

    /// `dart`, then `flutter`, on the `PATH`.
    fn on_path(&mut self) -> bool {
        let Some(path_var) = std::env::var_os("PATH") else {
            return false;
        };
        // An empty entry would mean the current directory: skipped.
        let directories: Vec<PathBuf> = std::env::split_paths(&path_var)
            .filter(|d| !d.as_os_str().is_empty())
            .collect();
        let dart: &[&str] = if cfg!(windows) {
            &["dart.exe", "dart.bat", "dart"]
        } else {
            &["dart"]
        };
        for dir in &directories {
            for exe in dart {
                let file = dir.join(exe);
                if !file.is_file() {
                    continue;
                }
                let source = format!("dart on the PATH ({})", file.display());
                // `<bin>/../lib` without the `..` (SDK paths show up in messages, as
                // in the analyzer: `…/dart-sdk/lib/…`); with a symlinked `dart`
                // (Homebrew), via the real executable; the Flutter wrapper keeps the
                // SDK in `<flutter>/bin/cache/dart-sdk`.
                let mut candidates: Vec<PathBuf> = Vec::new();
                if let Some(parent) = dir.parent() {
                    candidates.push(parent.to_path_buf());
                }
                candidates.push(dir.join(".."));
                candidates.push(dir.join("cache").join("dart-sdk"));
                if let Ok(real) = std::fs::canonicalize(&file)
                    && let Some(real_bin) = real.parent()
                {
                    if let Some(root) = real_bin.parent() {
                        candidates.push(root.to_path_buf());
                    }
                    candidates.push(real_bin.join("cache").join("dart-sdk"));
                }
                if self.first_valid(&source, &candidates) {
                    return true;
                }
            }
        }
        let flutter: &[&str] = if cfg!(windows) {
            &["flutter.bat", "flutter.exe", "flutter"]
        } else {
            &["flutter"]
        };
        for dir in &directories {
            for exe in flutter {
                let file = dir.join(exe);
                if !file.is_file() {
                    continue;
                }
                let source = format!("flutter on the PATH ({})", file.display());
                let mut candidates = vec![dir.join("cache").join("dart-sdk")];
                if let Ok(real) = std::fs::canonicalize(&file)
                    && let Some(real_bin) = real.parent()
                {
                    candidates.push(real_bin.join("cache").join("dart-sdk"));
                }
                if self.first_valid(&source, &candidates) {
                    return true;
                }
            }
        }
        false
    }

    /// The first valid of `candidates` that loads; when none is valid, one
    /// refusal for `source` (each candidate that failed to load is already
    /// listed on its own). The same SDK reached through several paths is
    /// tried once.
    fn first_valid(&mut self, source: &str, candidates: &[PathBuf]) -> bool {
        let mut tried: Vec<PathBuf> = Vec::new();
        for c in candidates {
            if let Ok(lib) = sdk_lib(c) {
                let key = lib.canonicalize().unwrap_or_else(|_| lib.clone());
                if tried.contains(&key) {
                    continue;
                }
                tried.push(key);
                if self.accept(source, lib) {
                    return true;
                }
            }
        }
        if tried.is_empty() {
            self.rejected
                .push(format!("{source}: no Dart SDK found around it"));
        }
        false
    }
}

/// `<flutter>/bin/cache/dart-sdk`.
fn flutter_sdk(flutter: &Path) -> PathBuf {
    flutter.join("bin").join("cache").join("dart-sdk")
}

fn non_empty_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Well-known install locations, most specific first.
fn known_locations() -> Vec<(String, PathBuf)> {
    let mut out: Vec<PathBuf> = Vec::new();
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    if cfg!(windows) {
        out.push(PathBuf::from(r"C:\tools\dart-sdk"));
        if let Some(p) = std::env::var_os("ProgramFiles") {
            out.push(Path::new(&p).join("Dart").join("dart-sdk"));
        }
        if let Some(p) = std::env::var_os("LOCALAPPDATA") {
            out.push(Path::new(&p).join("Dart").join("dart-sdk"));
        }
        out.push(flutter_sdk(Path::new(r"C:\src\flutter")));
        out.push(flutter_sdk(Path::new(r"C:\flutter")));
    } else {
        for prefix in [
            "/opt/homebrew/opt",
            "/usr/local/opt",
            "/home/linuxbrew/.linuxbrew/opt",
        ] {
            out.push(Path::new(prefix).join("dart@3.6").join("libexec"));
            out.push(Path::new(prefix).join("dart").join("libexec"));
        }
        out.push(PathBuf::from("/usr/lib/dart"));
        out.push(PathBuf::from("/opt/dart-sdk"));
        out.push(flutter_sdk(Path::new("/snap/flutter/common/flutter")));
        out.push(flutter_sdk(Path::new("/opt/flutter")));
    }
    if let Some(home) = home {
        out.push(home.join("dart-sdk"));
        out.push(flutter_sdk(&home.join("flutter")));
        out.push(flutter_sdk(&home.join("development").join("flutter")));
        out.push(flutter_sdk(&home.join("fvm").join("default")));
    }
    out.into_iter()
        .map(|p| ("a well-known install location".to_string(), p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_sdk(root: &Path, version: &str) {
        std::fs::create_dir_all(root.join("lib/core")).unwrap();
        std::fs::write(root.join("lib/libraries.json"), "{}").unwrap();
        std::fs::write(root.join("lib/core/core.dart"), "").unwrap();
        std::fs::write(root.join("version"), format!("{version}\n")).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sdk-discovery-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn accepts_root_or_lib_and_reads_the_version() {
        let d = scratch("root-or-lib");
        fake_sdk(&d, "3.6.2");
        assert_eq!(sdk_lib(&d).unwrap(), d.join("lib"));
        assert_eq!(sdk_lib(&d.join("lib")).unwrap(), d.join("lib"));
        assert_eq!(sdk_version(&d.join("lib")).as_deref(), Some("3.6.2"));
        std::fs::remove_file(d.join("lib/core/core.dart")).unwrap();
        assert!(sdk_lib(&d).unwrap_err().contains("core/core.dart"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_flutter_wrapper_leads_to_its_cached_sdk() {
        let d = scratch("flutter");
        std::fs::create_dir_all(d.join("bin")).unwrap();
        std::fs::write(d.join("bin/dart"), "#!/bin/sh\n").unwrap();
        fake_sdk(&flutter_sdk(&d), "3.6.2");
        let mut load = accept_all;
        let mut s = Search {
            found: None,
            rejected: Vec::new(),
            load: &mut load,
        };
        let bin = d.join("bin");
        assert!(s.first_valid("test", &[bin.join(".."), bin.join("cache/dart-sdk")]));
        assert_eq!(
            s.found.unwrap().lib,
            d.join("bin/cache/dart-sdk/lib"),
            "the wrapper's directory has no lib/; the cache has the SDK"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    fn accept_all(_: &Path) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn a_candidate_that_fails_to_load_falls_through_to_the_next() {
        let d = scratch("load-fails");
        fake_sdk(&d.join("broken"), "3.13.0");
        fake_sdk(&d.join("good"), "3.6.2");
        let broken_lib = d.join("broken/lib");
        let mut load = |lib: &Path| {
            if lib == broken_lib {
                Err("libraries.json is invalid".to_string())
            } else {
                Ok(())
            }
        };
        let mut s = Search {
            found: None,
            rejected: Vec::new(),
            load: &mut load,
        };
        assert!(!s.probe("DART_SDK", &d.join("broken"), true));
        assert!(s.probe("FLUTTER_ROOT", &d.join("good"), true));
        let found = s.found.clone().unwrap();
        assert_eq!(found.lib, d.join("good/lib"));
        assert_eq!(found.version.as_deref(), Some("3.6.2"));
        assert_eq!(s.rejected.len(), 1, "{:?}", s.rejected);
        assert!(
            s.rejected[0].starts_with("DART_SDK: ")
                && s.rejected[0].ends_with("(could not be loaded: libraries.json is invalid)"),
            "{:?}",
            s.rejected
        );
        // Several paths around one `dart`: the broken one is skipped, once.
        let mut s = Search {
            found: None,
            rejected: Vec::new(),
            load: &mut load,
        };
        let candidates = [d.join("broken"), d.join("broken/lib"), d.join("good")];
        assert!(s.first_valid("dart on the PATH", &candidates));
        assert_eq!(s.found.unwrap().lib, d.join("good/lib"));
        assert_eq!(s.rejected.len(), 1, "{:?}", s.rejected);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn named_candidates_are_reported_even_when_missing() {
        let mut load = accept_all;
        let mut s = Search {
            found: None,
            rejected: Vec::new(),
            load: &mut load,
        };
        assert!(!s.probe("DART_SDK", Path::new("/definitely/not/here"), true));
        assert!(!s.probe(
            "a well-known install location",
            Path::new("/nor/here"),
            false
        ));
        assert_eq!(s.rejected.len(), 1, "{:?}", s.rejected);
        assert!(s.rejected[0].starts_with("DART_SDK: /definitely/not/here"));
    }
}
