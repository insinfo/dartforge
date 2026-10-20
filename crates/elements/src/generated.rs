//! Generated Dart sources, kept in memory.
//!
//! An ngdart project imports `foo.template.dart`, which does not exist on
//! disk: `build_runner` writes it to `.dart_tool/build/generated`. Here those
//! sources live in memory, indexed by the **natural** path — the path next
//! to the file that produced them, `lib/src/x/foo.template.dart` — which is
//! exactly what `resolve_package_uri` returns for the imported URI. That way
//! nothing changes in resolution or in the mapping back to `package:`: only
//! reading consults this table before the disk.
//!
//! A generation is immutable and replaced whole at once. That is what
//! guarantees the browser never sees an intermediate state: either the
//! generation closed without errors and the JS module comes out consistent
//! with it, or the previous one stays in effect.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A Dart source produced by a generator.
#[derive(Debug, Clone)]
pub struct GeneratedSource {
    pub content: Arc<str>,
    /// Files that produced it (`.dart`, `.html`, `.css`): through them the
    /// `serve` watcher knows what to invalidate.
    pub inputs: Arc<[PathBuf]>,
    pub generator: &'static str,
}

/// Immutable set of generated sources.
#[derive(Debug, Default)]
pub struct Generation {
    pub id: u64,
    sources: HashMap<PathBuf, GeneratedSource>,
}

/// Normalizes the path to serve as a key: lexically (without touching the
/// disk, since the file does not exist) and without Windows' verbatim
/// prefix.
pub fn key(path: &Path) -> PathBuf {
    crate::config::without_verbatim(crate::load::normalize(path))
}

impl Generation {
    pub fn get(&self, path: &Path) -> Option<&GeneratedSource> {
        self.sources.get(&key(path))
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.sources.contains_key(&key(path))
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.sources.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &GeneratedSource)> {
        self.sources.iter()
    }
}

/// Accumulates a generation. It only becomes a `Generation` if it closes
/// without errors.
#[derive(Default)]
pub struct GenerationBuilder {
    sources: HashMap<PathBuf, GeneratedSource>,
    errors: Vec<String>,
}

impl GenerationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(
        &mut self,
        path: PathBuf,
        content: impl Into<Arc<str>>,
        generator: &'static str,
        inputs: Vec<PathBuf>,
    ) {
        self.sources.insert(
            key(&path),
            GeneratedSource {
                content: content.into(),
                inputs: inputs.into(),
                generator,
            },
        );
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.sources.contains_key(&key(path))
    }

    pub fn error(&mut self, message: String) {
        self.errors.push(message);
    }

    /// Closes the generation. With any error nothing is published — the caller
    /// keeps the previous generation.
    pub fn finish(self, id: u64) -> Result<Arc<Generation>, Vec<String>> {
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        Ok(Arc::new(Generation {
            id,
            sources: self.sources,
        }))
    }
}

/// Generation built from what `build_runner` already wrote to
/// `.dart_tool/build/generated`, mapping each file to its natural path next
/// to the source.
///
/// Used to verify the generator's separate plumbing: with the same input,
/// compiling while reading from here must give byte for byte the same JS as
/// compiling while reading from disk. When the Angular generator in Rust
/// arrives, only who produces the strings changes.
pub fn from_build_runner(
    cfg: &crate::config::PackageConfig,
    suffixes: &[&str],
    packages_: Option<&std::collections::HashSet<String>>,
) -> Arc<Generation> {
    let mut c = GenerationBuilder::new();
    let Some(origin) = cfg.origin.as_ref().and_then(|o| o.parent()) else {
        return c.finish(0).unwrap_or_default_arc();
    };
    let root = origin.join("build").join("generated");
    for (name_, pkg) in &cfg.packages {
        if packages_.is_some_and(|p| !p.contains(name_)) {
            continue;
        }
        let dir = root.join(name_).join("lib");
        if !dir.is_dir() {
            continue;
        }
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(inputs) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in inputs.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let name_ = p.to_string_lossy();
                if !suffixes.iter().any(|s| name_.ends_with(s)) {
                    continue;
                }
                let Ok(rel) = p.strip_prefix(&dir) else {
                    continue;
                };
                let rel = rel
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/");
                let Ok(destination) = pkg.package_uri.join(&rel).and_then(|u| {
                    u.to_file_path()
                        .map_err(|_| url::ParseError::RelativeUrlWithoutBase)
                }) else {
                    continue;
                };
                match std::fs::read_to_string(&p) {
                    Ok(text) => c.put(destination, text, "build_runner", vec![p]),
                    Err(e) => c.error(format!("could not read {}: {e}", p.display())),
                }
            }
        }
    }
    c.finish(0).unwrap_or_default_arc()
}

trait OrEmpty {
    fn unwrap_or_default_arc(self) -> Arc<Generation>;
}

impl OrEmpty for Result<Arc<Generation>, Vec<String>> {
    fn unwrap_or_default_arc(self) -> Arc<Generation> {
        self.unwrap_or_else(|e| {
            for m in e {
                eprintln!("warning: {m}");
            }
            Arc::new(Generation::default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_lexical() {
        let g = {
            let mut c = GenerationBuilder::new();
            c.put(
                PathBuf::from("/p/lib/./a/../foo.template.dart"),
                "// x",
                "teste",
                vec![],
            );
            c.finish(1).unwrap()
        };
        assert!(g.contains(Path::new("/p/lib/foo.template.dart")));
        assert_eq!(
            g.get(Path::new("/p/lib/foo.template.dart"))
                .unwrap()
                .content
                .as_ref(),
            "// x"
        );
        assert!(!g.contains(Path::new("/p/lib/other.dart")));
    }

    #[test]
    fn error_aborts_generation() {
        let mut c = GenerationBuilder::new();
        c.put(
            PathBuf::from("/p/lib/foo.template.dart"),
            "// x",
            "teste",
            vec![],
        );
        c.error("broken template".into());
        assert!(c.finish(2).is_err());
    }
}
